use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::broadcast;

use crate::utils::Sample;

pub fn setup_audio() -> Result<broadcast::Sender<Sample>> {
    let (audio_data_tx, _) = broadcast::channel(8);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("No output device found"))?;

    if let Ok(name) = device.name() {
        tracing::info!("Using audio device: {}", name);
    } else {
        tracing::info!("Using audio device: {:?}", device.name());
    }

    let audio_cfg = device.default_output_config()?;

    dbg!(&audio_cfg);

    let channel_count = audio_cfg.channels();
    let sample_rate = audio_cfg.sample_rate().0;

    // Target 10ms packet size
    let packet_size = (sample_rate as usize / 100) * channel_count as usize;
    let packet_duration = Duration::from_millis(10);

    let mut buffer = vec![0.0f32; packet_size];
    let mut buffer_filled = 0;

    let mut encoder = opus::Encoder::new(
        sample_rate,
        match channel_count {
            1 => opus::Channels::Mono,
            2 => opus::Channels::Stereo,
            _ => bail!("Unsupported channel count: {}", channel_count),
        },
        opus::Application::Voip,
    )?;

    let mut encoded_buffer = vec![0u8; packet_size * std::mem::size_of::<f32>()];

    let data_tx = audio_data_tx.clone();
    let stream = device.build_input_stream(
        &audio_cfg.config(),
        move |mut data: &[f32], _callback_info| {
            // if data_tx.receiver_count() == 0 {
            //     return;
            // }

            dbg!(data);

            let pts = std::time::SystemTime::now();

            while !data.is_empty() {
                let to_copy = std::cmp::min(data.len(), packet_size - buffer_filled);
                buffer[buffer_filled..buffer_filled + to_copy].copy_from_slice(&data[..to_copy]);
                buffer_filled += to_copy;
                data = &data[to_copy..];

                if buffer_filled == packet_size {
                    tracing::trace!("Got a full audio packet, encoding it");
                    match encoder.encode_f32(&buffer, &mut encoded_buffer) {
                        Ok(len) => {
                            let sample = Sample::new(&encoded_buffer[..len], pts, packet_duration);
                            data_tx.send(sample).ok();
                        }
                        Err(e) => {
                            tracing::error!(?e, "Failed to encode audio");
                        }
                    }
                    buffer_filled = 0;
                }
            }
        },
        |err| {
            tracing::error!(?err, "Audio stream error");
        },
    )?;

    stream.play()?;

    tracing::info!("Audio stream started");

    Ok(audio_data_tx)
}
