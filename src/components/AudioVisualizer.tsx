import { useEffect, useRef } from 'react';
import './AudioVisualizer.css';

interface AudioVisualizerProps {
  audioLevel: number;
  isRecording: boolean;
  showWaveform?: boolean;
}

const AudioVisualizer: React.FC<AudioVisualizerProps> = ({
  audioLevel,
  isRecording,
  showWaveform = true,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!showWaveform || !canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Simple animated waveform based on audio level
    const animate = () => {
      if (!isRecording) return;

      const width = canvas.width;
      const height = canvas.height;
      const centerY = height / 2;

      // Clear canvas
      ctx.fillStyle = '#1a1a1a';
      ctx.fillRect(0, 0, width, height);

      // Draw waveform
      ctx.strokeStyle = '#4a9eff';
      ctx.lineWidth = 2;
      ctx.beginPath();

      const amplitude = (audioLevel / 100) * (height / 2) * 0.8;
      const frequency = 0.02;
      const time = Date.now() * 0.001;

      for (let x = 0; x < width; x++) {
        const y =
          centerY +
          Math.sin(x * frequency + time * 2) * amplitude +
          Math.sin(x * frequency * 2 + time * 3) * (amplitude * 0.5);

        if (x === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      }

      ctx.stroke();

      requestAnimationFrame(animate);
    };

    if (isRecording) {
      animate();
    }
  }, [audioLevel, isRecording, showWaveform]);

  // Determine volume bar color based on level
  const getVolumeColor = (level: number): string => {
    if (level < 30) return '#4ade80'; // Green
    if (level < 60) return '#fbbf24'; // Yellow
    if (level < 80) return '#fb923c'; // Orange
    return '#ef4444'; // Red
  };

  return (
    <div className="audio-visualizer">
      {/* Volume meter bar */}
      <div className="volume-meter">
        <div className="volume-meter-label">Volume</div>
        <div className="volume-meter-bar-container">
          <div
            className="volume-meter-bar"
            style={{
              width: `${audioLevel}%`,
              backgroundColor: getVolumeColor(audioLevel),
              transition: 'width 0.1s ease-out',
            }}
          />
        </div>
        <div className="volume-meter-value">{Math.round(audioLevel)}%</div>
      </div>

      {/* Waveform visualization */}
      {showWaveform && (
        <div className="waveform-container">
          <canvas
            ref={canvasRef}
            width={200}
            height={50}
            className="waveform-canvas"
          />
        </div>
      )}

      {/* Pulsing indicator */}
      {isRecording && (
        <div className="recording-indicator">
          <div className="recording-pulse" />
          <span className="recording-text">Recording...</span>
        </div>
      )}
    </div>
  );
};

export default AudioVisualizer;
