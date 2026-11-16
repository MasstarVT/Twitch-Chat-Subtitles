//! Microphone capture. The audio callback only converts samples to mono f32
//! and hands them off; all heavy work happens on the recognizer thread.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use std::sync::mpsc::SyncSender;

pub struct Capture {
    _stream: cpal::Stream,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

fn device_name(d: &cpal::Device) -> String {
    d.description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "<unknown device>".into())
}

pub fn list_devices() -> Result<(), String> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| device_name(&d));
    let devices = host
        .input_devices()
        .map_err(|e| format!("Could not list microphones: {e}"))?;

    println!("Microphones:");
    for d in devices {
        let name = device_name(&d);
        let mark = if default.as_deref() == Some(name.as_str()) {
            "   <- default"
        } else {
            ""
        };
        let info = d
            .default_input_config()
            .map(|c| format!("{} Hz, {} ch", c.sample_rate(), c.channels()))
            .unwrap_or_default();
        println!("  {name}  [{info}]{mark}");
    }
    println!("\nPick one with --device \"<part of the name>\"");
    Ok(())
}

pub fn start(
    wanted: Option<&str>,
    channel: Option<usize>,
    tx: SyncSender<Vec<f32>>,
) -> Result<Capture, String> {
    let host = cpal::default_host();
    let device = match wanted {
        Some(w) => {
            let needle = w.to_lowercase();
            host.input_devices()
                .map_err(|e| format!("Could not list microphones: {e}"))?
                .find(|d| device_name(d).to_lowercase().contains(&needle))
                .ok_or_else(|| {
                    format!("No microphone matches \"{w}\". Run with --list-devices to see them.")
                })?
        }
        None => host
            .default_input_device()
            .ok_or("No default microphone found. Plug one in or pick one with --device.")?,
    };
    let name = device_name(&device);

    let supported = device
        .default_input_config()
        .map_err(|e| format!("Could not read the format of \"{name}\": {e}"))?;
    let format = supported.sample_format();
    let config = supported.config();
    let channels = config.channels;

    if let Some(c) = channel
        && c > channels as usize
    {
        return Err(format!(
            "\"{name}\" only has {channels} channel(s), so --channel {c} does not exist."
        ));
    }
    let pick = Pick {
        channels: channels as usize,
        only: channel.map(|c| c - 1),
    };

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, pick, tx),
        SampleFormat::I16 => build::<i16>(&device, &config, pick, tx),
        SampleFormat::I32 => build::<i32>(&device, &config, pick, tx),
        SampleFormat::U16 => build::<u16>(&device, &config, pick, tx),
        SampleFormat::U8 => build::<u8>(&device, &config, pick, tx),
        SampleFormat::I8 => build::<i8>(&device, &config, pick, tx),
        SampleFormat::F64 => build::<f64>(&device, &config, pick, tx),
        other => {
            return Err(format!(
                "\"{name}\" uses an unsupported sample format ({other:?})."
            ));
        }
    }
    .map_err(|e| format!("Could not open \"{name}\": {e}"))?;

    stream
        .play()
        .map_err(|e| format!("Could not start \"{name}\": {e}"))?;

    Ok(Capture {
        _stream: stream,
        name,
        sample_rate: config.sample_rate,
        channels,
    })
}

#[derive(Clone, Copy)]
struct Pick {
    channels: usize,
    /// Zero-based channel to keep, or `None` to average all channels.
    only: Option<usize>,
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    pick: Pick,
    tx: SyncSender<Vec<f32>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut dropped = 0u64;
    device.build_input_stream::<T, _, _>(
        *config,
        move |data: &[T], _| {
            let mono = to_mono(data, pick);
            // Never block the audio thread. If recognition falls behind, drop audio.
            if tx.try_send(mono).is_err() {
                dropped += 1;
                if dropped.is_power_of_two() {
                    eprintln!("[audio] recognizer is falling behind, dropped {dropped} buffers");
                }
            }
        },
        |e: cpal::Error| match e.kind() {
            // A brief glitch (common right after the stream opens) doesn't matter for speech.
            cpal::ErrorKind::Xrun => {}
            cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => {
                eprintln!(
                    "[audio] The microphone was disconnected. Plug it back in and restart the app."
                )
            }
            _ => eprintln!("[audio] {e}"),
        },
        None,
    )
}

fn to_mono<T>(data: &[T], pick: Pick) -> Vec<f32>
where
    T: Sample,
    f32: FromSample<T>,
{
    let frames = data.chunks_exact(pick.channels);
    match pick.only {
        _ if pick.channels == 1 => data.iter().map(|&s| f32::from_sample(s)).collect(),
        Some(c) => frames.map(|f| f32::from_sample(f[c])).collect(),
        None => {
            let n = pick.channels as f32;
            frames
                .map(|f| f.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / n)
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_and_picks_channels() {
        let stereo = [1.0f32, 0.0, 0.5, 0.5];
        assert_eq!(
            to_mono(
                &stereo,
                Pick {
                    channels: 2,
                    only: None
                }
            ),
            vec![0.5, 0.5]
        );
        assert_eq!(
            to_mono(
                &stereo,
                Pick {
                    channels: 2,
                    only: Some(0)
                }
            ),
            vec![1.0, 0.5]
        );
        assert_eq!(
            to_mono(
                &stereo,
                Pick {
                    channels: 2,
                    only: Some(1)
                }
            ),
            vec![0.0, 0.5]
        );
    }

    #[test]
    fn converts_integer_samples() {
        let out = to_mono(
            &[i16::MAX, 0],
            Pick {
                channels: 1,
                only: None,
            },
        );
        assert!((out[0] - 1.0).abs() < 1e-3 && out[1] == 0.0);
    }
}
