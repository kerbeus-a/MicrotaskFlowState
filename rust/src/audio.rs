//! Microphone capture, resampling, and the check-in chime.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_RECORDING_SECS: usize = 300;
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

pub fn input_device_names() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// An in-progress microphone recording. Dropping it stops capture.
pub struct Recording {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    level: Arc<Mutex<f32>>,
    sample_rate: u32,
    pub started: Instant,
}

/// Captured mono audio at its native sample rate.
pub struct Captured {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Captured {
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }

    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// Linear-interpolation resample to 16 kHz mono for Whisper.
    pub fn to_whisper_rate(&self) -> Vec<f32> {
        let input_rate = self.sample_rate as f32;
        let output_rate = WHISPER_SAMPLE_RATE as f32;
        if (input_rate - output_rate).abs() < 1.0 {
            return self.samples.clone();
        }
        let data = &self.samples;
        let ratio = input_rate / output_rate;
        let new_len = (data.len() as f32 / ratio) as usize;
        (0..new_len)
            .map(|i| {
                let src = i as f32 * ratio;
                let idx = src as usize;
                let frac = src - idx as f32;
                match (data.get(idx), data.get(idx + 1)) {
                    (Some(a), Some(b)) => a * (1.0 - frac) + b * frac,
                    (Some(a), None) => *a,
                    _ => 0.0,
                }
            })
            .collect()
    }
}

impl Recording {
    /// Start capturing from the named device, or the system default when `device_name` is None
    /// or no longer present.
    pub fn start(device_name: Option<&str>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = device_name
            .and_then(|name| {
                host.input_devices()
                    .ok()?
                    .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            })
            .or_else(|| host.default_input_device())
            .ok_or("No microphone found")?;

        let supported = device
            .default_input_config()
            .map_err(|e| format!("Microphone unavailable: {e}"))?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let max_samples = MAX_RECORDING_SECS.saturating_mul(sample_rate as usize);
        let config: cpal::StreamConfig = supported.into();

        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let level = Arc::new(Mutex::new(0.0f32));

        // Shared sink: downmix to mono, track level, append up to the cap.
        let make_sink = || {
            let buffer = buffer.clone();
            let level = level.clone();
            move |mono: Vec<f32>| {
                if !mono.is_empty() {
                    let rms = (mono.iter().map(|s| s * s).sum::<f32>() / mono.len() as f32).sqrt();
                    let l = (rms * 4.0).min(1.0);
                    if let Ok(mut lvl) = level.lock() {
                        *lvl = if l > *lvl { l } else { *lvl * 0.85 + l * 0.15 };
                    }
                }
                let mut buf = buffer.lock().unwrap();
                let remaining = max_samples.saturating_sub(buf.len());
                buf.extend(mono.into_iter().take(remaining));
            }
        };
        let on_err = |err| eprintln!("Audio error: {}", err);

        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let sink = make_sink();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        sink(data
                            .chunks(channels)
                            .map(|c| c.iter().sum::<f32>() / channels as f32)
                            .collect())
                    },
                    on_err,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let sink = make_sink();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        sink(data
                            .chunks(channels)
                            .map(|c| c.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32)
                            .collect())
                    },
                    on_err,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let sink = make_sink();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        sink(data
                            .chunks(channels)
                            .map(|c| {
                                c.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum::<f32>()
                                    / channels as f32
                            })
                            .collect())
                    },
                    on_err,
                    None,
                )
            }
            other => return Err(format!("Unsupported microphone format: {other:?}")),
        }
        .map_err(|e| format!("Failed to open microphone: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start microphone: {e}"))?;

        Ok(Self {
            _stream: stream,
            buffer,
            level,
            sample_rate,
            started: Instant::now(),
        })
    }

    pub fn level(&self) -> f32 {
        self.level.lock().map(|l| *l).unwrap_or(0.0)
    }

    pub fn finish(self) -> Captured {
        let samples = std::mem::take(&mut *self.buffer.lock().unwrap());
        Captured {
            samples,
            sample_rate: self.sample_rate,
        }
    }
}

/// Play a soft 3-tone chime (C5/E5/G5) on the default output device, on a background thread.
pub fn play_chime() {
    thread::spawn(|| {
        if let Err(e) = play_chime_blocking() {
            eprintln!("⚠️ chime: {e}");
        }
    });
}

fn play_chime_blocking() -> Result<(), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no default output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_rate = supported.sample_rate().0 as f32;
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();

    let tones: [f32; 3] = [523.25, 659.25, 783.99];
    let tone_samples = (sample_rate * 0.22) as usize;
    let gap_samples = (sample_rate * 0.04) as usize;
    let attack_samples = (sample_rate * 0.02) as usize;
    let peak_gain: f32 = 0.3;

    // Pre-render so the audio callback only copies samples.
    let mut buf: Vec<f32> = Vec::new();
    for &freq in &tones {
        let w = 2.0 * std::f32::consts::PI * freq / sample_rate;
        for i in 0..tone_samples {
            let env = if i < attack_samples {
                peak_gain * (i as f32 / attack_samples as f32)
            } else {
                let pos = (i - attack_samples) as f32 / (tone_samples - attack_samples).max(1) as f32;
                peak_gain * (-4.0 * pos).exp()
            };
            let s = env * (w * i as f32).sin();
            buf.extend(std::iter::repeat(s).take(channels));
        }
        buf.extend(std::iter::repeat(0.0).take(gap_samples * channels));
    }

    let total_frames = buf.len() / channels.max(1);
    let buf = Arc::new(buf);
    let cursor = Arc::new(Mutex::new(0usize));

    fn next_sample(buf: &[f32], cursor: &Mutex<usize>) -> f32 {
        let mut idx = cursor.lock().unwrap();
        let s = buf.get(*idx).copied().unwrap_or(0.0);
        *idx += 1;
        s
    }

    let on_err = |err| eprintln!("⚠️ chime stream error: {}", err);
    let (b, c) = (buf.clone(), cursor.clone());
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                data.iter_mut().for_each(|s| *s = next_sample(&b, &c))
            },
            on_err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                data.iter_mut()
                    .for_each(|s| *s = (next_sample(&b, &c) * i16::MAX as f32) as i16)
            },
            on_err,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                data.iter_mut().for_each(|s| {
                    *s = ((next_sample(&b, &c) * 0.5 + 0.5) * u16::MAX as f32) as u16
                })
            },
            on_err,
            None,
        ),
        other => return Err(format!("unsupported sample format {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_secs_f32(total_frames as f32 / sample_rate) + Duration::from_millis(150));
    Ok(())
}
