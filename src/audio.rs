use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg_attr(test, mockall::automock)]
pub trait AudioCapture {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<Vec<u8>>;
    fn is_recording(&self) -> bool;
}

pub struct CpalAudioCapture {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    recording: Arc<Mutex<bool>>,
    timeout_handle: Option<tokio::task::JoinHandle<()>>,
    sample_rate: u32,
}

impl CpalAudioCapture {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            recording: Arc::new(Mutex::new(false)),
            timeout_handle: None,
            sample_rate: 16000,
        }
    }
}

impl AudioCapture for CpalAudioCapture {
    fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No default input device found"))?;

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        self.sample_rate = config.sample_rate().0;
        
        {
            let mut samples = self.samples.lock().unwrap();
            samples.clear();
        }
        {
            let mut recording = self.recording.lock().unwrap();
            *recording = true;
        }

        let samples_clone = Arc::clone(&self.samples);
        let recording_clone = Arc::clone(&self.recording);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let config: cpal::StreamConfig = config.into();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if *recording_clone.lock().unwrap() {
                            let mut samples = samples_clone.lock().unwrap();
                            samples.extend_from_slice(data);
                        }
                    },
                    |err| tracing::error!("Audio stream error: {}", err),
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let config: cpal::StreamConfig = config.into();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if *recording_clone.lock().unwrap() {
                            let mut samples = samples_clone.lock().unwrap();
                            samples.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                        }
                    },
                    |err| tracing::error!("Audio stream error: {}", err),
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let config: cpal::StreamConfig = config.into();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if *recording_clone.lock().unwrap() {
                            let mut samples = samples_clone.lock().unwrap();
                            samples.extend(data.iter().map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0));
                        }
                    },
                    |err| tracing::error!("Audio stream error: {}", err),
                    None,
                )?
            }
            _ => bail!("Unsupported sample format: {:?}", config.sample_format()),
        };

        stream.play().context("Failed to start audio stream")?;
        self.stream = Some(stream);

        let recording_timeout = Arc::clone(&self.recording);
        let timeout_handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let mut recording = recording_timeout.lock().unwrap();
            *recording = false;
            tracing::warn!("Audio recording timed out after 30 seconds");
        });
        self.timeout_handle = Some(timeout_handle);

        Ok(())
    }

    fn stop(&mut self) -> Result<Vec<u8>> {
        {
            let mut recording = self.recording.lock().unwrap();
            *recording = false;
        }

        if let Some(handle) = self.timeout_handle.take() {
            handle.abort();
        }

        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        let samples = {
            let samples = self.samples.lock().unwrap();
            samples.clone()
        };

        if !samples.is_empty() {
            let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
            if rms < 0.01 {
                bail!("Audio is silent (RMS amplitude: {:.6})", rms);
            }
        } else {
            bail!("No audio samples captured");
        }

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut wav_buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut wav_buffer, spec)
                .context("Failed to create WAV writer")?;

            for &sample in &samples {
                let sample_i16 = (sample.clamp(-1.0_f32, 1.0_f32) * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16)
                    .context("Failed to write WAV sample")?;
            }

            writer.finalize().context("Failed to finalize WAV")?;
        }

        Ok(wav_buffer.into_inner())
    }

    fn is_recording(&self) -> bool {
        *self.recording.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_encoding_with_known_samples() {
        let samples = vec![0.5, -0.5, 0.25, -0.25];
        let sample_rate = 16000;

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut wav_buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut wav_buffer, spec).unwrap();
            for &sample in &samples {
                let sample_f32: f32 = sample;
                let sample_i16 = (sample_f32.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16).unwrap();
            }
            writer.finalize().unwrap();
        }

        let wav_bytes = wav_buffer.into_inner();
        assert!(!wav_bytes.is_empty());
        assert!(wav_bytes.len() > 44);

        let reader = hound::WavReader::new(std::io::Cursor::new(wav_bytes)).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, sample_rate);
        assert_eq!(reader.spec().bits_per_sample, 16);
    }

    #[test]
    fn test_silence_detection() {
        let silent_samples = vec![0.001, -0.002, 0.0015, -0.0005];
        let rms = (silent_samples.iter().map(|&s| s * s).sum::<f32>() / silent_samples.len() as f32).sqrt();
        assert!(rms < 0.01, "Silent samples should have RMS < 0.01");

        let audible_samples = vec![0.5, -0.5, 0.3, -0.3];
        let rms = (audible_samples.iter().map(|&s| s * s).sum::<f32>() / audible_samples.len() as f32).sqrt();
        assert!(rms >= 0.01, "Audible samples should have RMS >= 0.01");
    }

    #[test]
    fn test_new_creates_empty_capture() {
        let capture = CpalAudioCapture::new();
        assert!(!capture.is_recording());
        assert_eq!(capture.sample_rate, 16000);
    }

    #[test]
    fn test_is_recording_reflects_state() {
        let capture = CpalAudioCapture::new();
        assert!(!capture.is_recording());

        {
            let mut recording = capture.recording.lock().unwrap();
            *recording = true;
        }
        assert!(capture.is_recording());

        {
            let mut recording = capture.recording.lock().unwrap();
            *recording = false;
        }
        assert!(!capture.is_recording());
    }
}
