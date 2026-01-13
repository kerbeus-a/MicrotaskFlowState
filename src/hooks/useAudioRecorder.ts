import { useState, useRef, useCallback, useEffect } from 'react';

export interface AudioDevice {
  deviceId: string;
  label: string;
}

export interface AudioRecorderState {
  isRecording: boolean;
  isPaused: boolean;
  recordingTime: number;
  audioLevel: number;
}

export interface UseAudioRecorderReturn {
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<Blob>;
  pauseRecording: () => void;
  resumeRecording: () => void;
  state: AudioRecorderState;
  error: string | null;
  // Device management
  availableDevices: AudioDevice[];
  selectedDeviceId: string | null;
  setSelectedDeviceId: (deviceId: string) => void;
  refreshDevices: () => Promise<void>;
}

// WAV file encoder
class WavEncoder {
  static encodeWAV(samples: Float32Array, sampleRate: number): Blob {
    const buffer = new ArrayBuffer(44 + samples.length * 2);
    const view = new DataView(buffer);

    // WAV header
    const writeString = (offset: number, string: string) => {
      for (let i = 0; i < string.length; i++) {
        view.setUint8(offset + i, string.charCodeAt(i));
      }
    };

    writeString(0, 'RIFF');
    view.setUint32(4, 36 + samples.length * 2, true);
    writeString(8, 'WAVE');
    writeString(12, 'fmt ');
    view.setUint32(16, 16, true); // PCM chunk size
    view.setUint16(20, 1, true); // PCM format
    view.setUint16(22, 1, true); // Mono
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, sampleRate * 2, true); // Byte rate
    view.setUint16(32, 2, true); // Block align
    view.setUint16(34, 16, true); // Bits per sample
    writeString(36, 'data');
    view.setUint32(40, samples.length * 2, true);

    // Write PCM samples
    let offset = 44;
    for (let i = 0; i < samples.length; i++) {
      const s = Math.max(-1, Math.min(1, samples[i]));
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
      offset += 2;
    }

    return new Blob([buffer], { type: 'audio/wav' });
  }

  static downsampleBuffer(
    buffer: Float32Array,
    inputSampleRate: number,
    outputSampleRate: number
  ): Float32Array {
    if (inputSampleRate === outputSampleRate) {
      return buffer;
    }

    const sampleRateRatio = inputSampleRate / outputSampleRate;
    const newLength = Math.round(buffer.length / sampleRateRatio);
    const result = new Float32Array(newLength);

    let offsetResult = 0;
    let offsetBuffer = 0;

    while (offsetResult < result.length) {
      const nextOffsetBuffer = Math.round((offsetResult + 1) * sampleRateRatio);
      let accum = 0;
      let count = 0;

      for (let i = offsetBuffer; i < nextOffsetBuffer && i < buffer.length; i++) {
        accum += buffer[i];
        count++;
      }

      result[offsetResult] = accum / count;
      offsetResult++;
      offsetBuffer = nextOffsetBuffer;
    }

    return result;
  }
}

export const useAudioRecorder = (): UseAudioRecorderReturn => {
  const [state, setState] = useState<AudioRecorderState>({
    isRecording: false,
    isPaused: false,
    recordingTime: 0,
    audioLevel: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [availableDevices, setAvailableDevices] = useState<AudioDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const animationFrameRef = useRef<number | null>(null);
  const startTimeRef = useRef<number>(0);
  const timerIntervalRef = useRef<number | null>(null);
  const isRecordingRef = useRef<boolean>(false);
  const isPausedRef = useRef<boolean>(false);

  // Load available audio input devices
  const refreshDevices = useCallback(async () => {
    try {
      // Request permission first to get device labels
      await navigator.mediaDevices.getUserMedia({ audio: true })
        .then(stream => stream.getTracks().forEach(track => track.stop()));
      
      const devices = await navigator.mediaDevices.enumerateDevices();
      const audioInputs = devices
        .filter(device => device.kind === 'audioinput')
        .map(device => ({
          deviceId: device.deviceId,
          label: device.label || `Microphone ${device.deviceId.slice(0, 8)}`,
        }));
      
      setAvailableDevices(audioInputs);
      
      // Set default device if none selected
      if (!selectedDeviceId && audioInputs.length > 0) {
        // Try to find 'default' device, otherwise use first
        const defaultDevice = audioInputs.find(d => d.deviceId === 'default') || audioInputs[0];
        setSelectedDeviceId(defaultDevice.deviceId);
      }
    } catch (err) {
      console.error('Failed to enumerate audio devices:', err);
    }
  }, [selectedDeviceId]);

  // Load devices on mount
  useEffect(() => {
    refreshDevices();
    
    // Listen for device changes (e.g., plugging in a new microphone)
    const handleDeviceChange = () => {
      refreshDevices();
    };
    navigator.mediaDevices.addEventListener('devicechange', handleDeviceChange);
    
    return () => {
      navigator.mediaDevices.removeEventListener('devicechange', handleDeviceChange);
    };
  }, [refreshDevices]);

  const updateAudioLevel = useCallback(() => {
    if (!analyserRef.current) {
      console.warn('updateAudioLevel: analyser not available');
      return;
    }

    // Use time domain data for more accurate volume detection
    const bufferLength = analyserRef.current.fftSize;
    const dataArray = new Uint8Array(bufferLength);
    analyserRef.current.getByteTimeDomainData(dataArray);

    // Calculate RMS (Root Mean Square) for accurate volume
    let sum = 0;
    let maxSample = 0;
    let minSample = 255;
    let nonSilentSamples = 0;
    
    for (let i = 0; i < bufferLength; i++) {
      const sample = dataArray[i];
      maxSample = Math.max(maxSample, sample);
      minSample = Math.min(minSample, sample);
      
      // Count samples that deviate from silence (128)
      if (Math.abs(sample - 128) > 1) {
        nonSilentSamples++;
      }
      
      const normalized = (sample - 128) / 128; // Normalize to -1 to 1
      sum += normalized * normalized;
    }
    const rms = Math.sqrt(sum / bufferLength);
    
    // Calculate peak-to-peak amplitude
    const peakToPeak = maxSample - minSample;
    const peakAmplitude = peakToPeak / 255; // Normalize to 0-1
    
    // Use both RMS and peak for better detection
    // RMS is better for average level, peak is better for detecting any sound
    const rmsLevel = Math.min(100, Math.max(0, (rms * 1000))); // More sensitive multiplier
    const peakLevel = Math.min(100, Math.max(0, (peakAmplitude * 200)));
    
    // Use the maximum of RMS and peak, with a minimum threshold
    const normalizedLevel = Math.max(rmsLevel, peakLevel * 0.5);

    // Debug logging (log more frequently during recording to diagnose)
    const shouldLog = Math.random() < 0.05 || normalizedLevel > 1; // Log 5% of time or when there's actual audio
    if (shouldLog) {
      console.log('Audio level:', {
        rms: rms.toFixed(4),
        peakAmplitude: peakAmplitude.toFixed(4),
        peakToPeak,
        rmsLevel: rmsLevel.toFixed(1),
        peakLevel: peakLevel.toFixed(1),
        normalizedLevel: normalizedLevel.toFixed(1),
        sampleRange: `${minSample}-${maxSample}`,
        nonSilentSamples: `${nonSilentSamples}/${bufferLength}`,
        bufferLength,
      });
      
      // Warn if we're getting silence
      if (peakToPeak < 5 && nonSilentSamples < bufferLength * 0.01) {
        console.warn('⚠️ Very low audio signal detected - microphone may not be receiving audio data');
      }
    }

    setState((prev) => ({ ...prev, audioLevel: normalizedLevel }));

    // Continue animation frame if still recording (use refs to avoid stale closure)
    if (isRecordingRef.current && !isPausedRef.current) {
      animationFrameRef.current = requestAnimationFrame(updateAudioLevel);
    }
  }, []);

  const startRecording = useCallback(async () => {
    try {
      setError(null);

      // Build audio constraints with selected device
      // Start with minimal constraints to avoid issues
      const audioConstraints: MediaTrackConstraints = {
        channelCount: 1,
        // Don't force sampleRate - let browser choose, we'll resample later
        echoCancellation: false, // Disable to avoid processing that might interfere
        noiseSuppression: false,
        autoGainControl: false,
      };

      // Add device ID if one is selected (use 'ideal' instead of 'exact' for better compatibility)
      if (selectedDeviceId && selectedDeviceId !== 'default') {
        audioConstraints.deviceId = selectedDeviceId; // Use ideal instead of exact
      }

      // Request microphone access
      let stream: MediaStream;
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          audio: audioConstraints,
        });
      } catch (err) {
        // If exact device fails, try without device constraint
        if (selectedDeviceId && selectedDeviceId !== 'default') {
          console.warn('Failed with specific device, trying default device...');
          stream = await navigator.mediaDevices.getUserMedia({
            audio: {
              channelCount: 1,
              echoCancellation: false,
              noiseSuppression: false,
              autoGainControl: false,
            },
          });
        } else {
          throw err;
        }
      }

      streamRef.current = stream;

      // Verify stream is active and force enable if needed
      const audioTracks = stream.getAudioTracks();
      console.log('Audio tracks:', audioTracks.length);
      audioTracks.forEach((track, index) => {
        console.log(`Track ${index} (before fix):`, {
          enabled: track.enabled,
          muted: track.muted,
          readyState: track.readyState,
          label: track.label,
          settings: track.getSettings(),
        });
        
        // Force enable the track (Kaspersky might disable it)
        if (!track.enabled) {
          console.warn(`Track ${index} is disabled, attempting to enable...`);
          track.enabled = true;
        }
        
        // Monitor mute state changes (Kaspersky might mute it after creation)
        track.onmute = () => {
          console.error(`Track ${index} was muted! This is likely Kaspersky blocking.`);
          setError('Microphone was muted. Please allow microphone access in Kaspersky and check "Remember my choice".');
        };
        
        track.onunmute = () => {
          console.log(`Track ${index} was unmuted.`);
        };
        
        // Check again after a short delay (Kaspersky might mute it asynchronously)
        setTimeout(() => {
          console.log(`Track ${index} (after delay):`, {
            enabled: track.enabled,
            muted: track.muted,
            readyState: track.readyState,
          });
          if (track.muted) {
            console.error('Track is muted - Kaspersky is likely blocking microphone access');
            setError('Microphone is muted. Please allow microphone access in Kaspersky and check "Remember my choice for this sequence".');
          }
        }, 500);
      });

      // Create audio context for visualization (use default sample rate for recording)
      // We'll downsample to 16kHz later when encoding WAV
      const audioContext = new AudioContext();
      audioContextRef.current = audioContext;
      
      // Resume audio context if suspended (browser autoplay policy)
      if (audioContext.state === 'suspended') {
        await audioContext.resume();
        console.log('AudioContext resumed from suspended state');
      }

      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 2048;
      analyser.smoothingTimeConstant = 0.3; // Lower smoothing for more responsive visualization
      source.connect(analyser);
      analyserRef.current = analyser;
      
      console.log('Analyser connected, fftSize:', analyser.fftSize, 'frequencyBinCount:', analyser.frequencyBinCount, 'AudioContext state:', audioContext.state);

      // Create MediaRecorder
      const mediaRecorder = new MediaRecorder(stream, {
        mimeType: 'audio/webm;codecs=opus',
      });

      chunksRef.current = [];

      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      mediaRecorder.start(100); // Collect data every 100ms
      mediaRecorderRef.current = mediaRecorder;

      startTimeRef.current = Date.now();

      // Start timer
      timerIntervalRef.current = window.setInterval(() => {
        setState((prev) => ({
          ...prev,
          recordingTime: Math.floor((Date.now() - startTimeRef.current) / 1000),
        }));
        
        // Check if track got muted (Kaspersky might mute it after recording starts)
        const tracks = streamRef.current?.getAudioTracks() || [];
        tracks.forEach((track, idx) => {
          if (track.muted && track.readyState === 'live') {
            console.warn(`Track ${idx} is muted during recording - Kaspersky may have blocked it`);
            // Try to unmute (may not work if Kaspersky is blocking)
            try {
              track.enabled = true;
            } catch (e) {
              console.error('Failed to re-enable track:', e);
            }
          }
        });
      }, 1000);

      setState((prev) => ({
        ...prev,
        isRecording: true,
        isPaused: false,
        recordingTime: 0,
      }));

      // Update refs
      isRecordingRef.current = true;
      isPausedRef.current = false;

      // Start audio level monitoring
      updateAudioLevel();
    } catch (err) {
      let errorMessage = err instanceof Error ? err.message : 'Failed to access microphone';
      
      // Provide more helpful error messages
      if (errorMessage.includes('Permission denied') || errorMessage.includes('NotAllowedError')) {
        errorMessage = 'Microphone permission denied. Please:\n1. Allow microphone access in Kaspersky (check "Remember my choice")\n2. Check Windows Privacy settings for microphone access';
      } else if (errorMessage.includes('NotFoundError') || errorMessage.includes('not found')) {
        errorMessage = 'Microphone not found. Please select a microphone in Settings.';
      } else if (errorMessage.includes('NotReadableError') || errorMessage.includes('could not be started')) {
        errorMessage = 'Microphone is in use or blocked. Check:\n1. Kaspersky is allowing access\n2. No other app is using the microphone\n3. Try the "Test Mic" button';
      }
      
      setError(errorMessage);
      console.error('Error starting recording:', err);
      throw err;
    }
  }, [updateAudioLevel, selectedDeviceId]);

  const stopRecording = useCallback(async (): Promise<Blob> => {
    return new Promise((resolve, reject) => {
      if (!mediaRecorderRef.current || !audioContextRef.current) {
        reject(new Error('No active recording'));
        return;
      }

      mediaRecorderRef.current.onstop = async () => {
        try {
          // Stop audio level monitoring
          if (animationFrameRef.current) {
            cancelAnimationFrame(animationFrameRef.current);
          }
          if (timerIntervalRef.current) {
            clearInterval(timerIntervalRef.current);
          }

          // Get recorded audio
          const audioBlob = new Blob(chunksRef.current, { type: 'audio/webm' });

          // Convert to WAV - need a fresh AudioContext for decoding
          // (the original one may have been created with different sample rate)
          const decodeContext = new AudioContext();
          const arrayBuffer = await audioBlob.arrayBuffer();

          let audioBuffer: AudioBuffer;
          try {
            audioBuffer = await decodeContext.decodeAudioData(arrayBuffer);
          } catch (decodeErr) {
            console.error('Failed to decode audio:', decodeErr);
            await decodeContext.close();
            throw new Error('Failed to decode audio. Try recording again.');
          }

          // Get audio data (first channel, mono)
          const channelData = audioBuffer.getChannelData(0);

          // Downsample to 16kHz if needed
          const targetSampleRate = 16000;
          const downsampledData = WavEncoder.downsampleBuffer(
            channelData,
            audioBuffer.sampleRate,
            targetSampleRate
          );

          // Encode as WAV
          const wavBlob = WavEncoder.encodeWAV(downsampledData, targetSampleRate);

          // Cleanup
          streamRef.current?.getTracks().forEach((track) => track.stop());
          await decodeContext.close();
          if (audioContextRef.current) {
            await audioContextRef.current.close();
          }

          // Update refs
          isRecordingRef.current = false;
          isPausedRef.current = false;

          setState({
            isRecording: false,
            isPaused: false,
            recordingTime: 0,
            audioLevel: 0,
          });

          chunksRef.current = [];
          mediaRecorderRef.current = null;
          audioContextRef.current = null;
          analyserRef.current = null;
          streamRef.current = null;

          resolve(wavBlob);
        } catch (err) {
          const errorMessage =
            err instanceof Error ? err.message : 'Failed to process recording';
          setError(errorMessage);
          reject(err);
        }
      };

      mediaRecorderRef.current.stop();
    });
  }, []);

  const pauseRecording = useCallback(() => {
    if (mediaRecorderRef.current && isRecordingRef.current && !isPausedRef.current) {
      mediaRecorderRef.current.pause();
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      isPausedRef.current = true;
      setState((prev) => ({ ...prev, isPaused: true }));
    }
  }, []);

  const resumeRecording = useCallback(() => {
    if (mediaRecorderRef.current && isRecordingRef.current && isPausedRef.current) {
      mediaRecorderRef.current.resume();
      isPausedRef.current = false;
      updateAudioLevel();
      setState((prev) => ({ ...prev, isPaused: false }));
    }
  }, [updateAudioLevel]);

  return {
    startRecording,
    stopRecording,
    pauseRecording,
    resumeRecording,
    state,
    error,
    // Device management
    availableDevices,
    selectedDeviceId,
    setSelectedDeviceId,
    refreshDevices,
  };
};
