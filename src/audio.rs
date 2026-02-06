use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use anyhow::Context;

pub fn record_audio() -> anyhow::Result<Vec<u8>> {
    let host = cpal::default_host();
    let device = host.default_input_device().context("No input device found")?;
    let config = device.default_input_config()?;

    println!("Input device: {}", device.name().unwrap_or("unknown".to_string()));

    let samples = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.clone().into(),
            move |data: &[f32], _: &_| {
                let mut s = samples_clone.lock().unwrap();
                for &sample in data {
                    let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    s.push(sample_i16);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.clone().into(),
            move |data: &[i16], _: &_| {
                let mut s = samples_clone.lock().unwrap();
                s.extend_from_slice(data);
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.clone().into(),
            move |data: &[u16], _: &_| {
                let mut s = samples_clone.lock().unwrap();
                for &sample in data {
                     let sample_i16 = (sample as i32 - 32768) as i16;
                     s.push(sample_i16);
                }
            },
            err_fn,
            None,
        )?,
        format => return Err(anyhow::anyhow!("Unsupported sample format: {:?}", format)),
    };

    stream.play()?;
    println!("Recording... Press Enter to stop.");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    drop(stream);

    let samples_data = samples.lock().unwrap();

    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for &sample in samples_data.iter() {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
    }

    Ok(cursor.into_inner())
}
