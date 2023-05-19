use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use bytes::{BufMut, Bytes, BytesMut};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream,
};
use crossbeam::channel;

use tokio::sync::watch;
use windows::{
    core::PCWSTR,
    Win32::{
        Media::Audio::{
            eRender, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
            IMMNotificationClient_Impl, MMDeviceEnumerator,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_DISABLE_OLE1DDE,
            COINIT_MULTITHREADED,
        },
    },
};

use crate::utils::Sample;

#[derive(Debug, Clone)]
pub enum AudioCodecData {
    Opus { ident_header: Bytes },
}

impl AudioCodecData {
    pub fn mime(&self) -> &'static str {
        match self {
            AudioCodecData::Opus { .. } => "audio/opus",
        }
    }
}

#[windows::core::implement(IMMNotificationClient)]
struct AudioNotificationClient {
    default_device_changed_tx: channel::Sender<()>,
}

impl IMMNotificationClient_Impl for AudioNotificationClient {
    fn OnDeviceStateChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _dwnewstate: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        _role: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender {
            self.default_device_changed_tx.try_send(()).ok();
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

fn audio_thread(audio_codec_data_tx: watch::Sender<Option<AudioCodecData>>) -> Result<()> {
    let data_tx = crate::get_app().audio_data_tx.clone();

    let enumerator: IMMDeviceEnumerator = unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE)?;
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?
    };

    let (default_device_changed_tx, default_device_changed_rx) = channel::bounded::<()>(1);
    default_device_changed_tx.send(()).ok(); // Trigger initial device change
    let callback: IMMNotificationClient = AudioNotificationClient {
        default_device_changed_tx,
    }
    .into();
    unsafe {
        enumerator.RegisterEndpointNotificationCallback(&callback)?;
    }

    let host = cpal::default_host();

    let mut current_stream: Option<Stream> = None;

    while default_device_changed_rx.recv().is_ok() {
        let _ = current_stream.take(); // Drop the old stream

        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No output device found"))?;

        tracing::info!(name = ?device.name(), "Using audio device");

        let audio_cfg = device.default_output_config()?;

        let channel_count = audio_cfg.channels();
        let sample_rate = audio_cfg.sample_rate().0;

        tracing::info!(channel_count, sample_rate, "Using default output config");

        // Target 20ms packet size
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
            opus::Application::Audio,
        )?;

        let mut header = BytesMut::new();
        header.put_slice(b"OpusHead");
        header.put_u8(1); // Version
        header.put_u8(channel_count as u8); // Channel count
        header.put_u16(0); // Pre-skip
        header.put_u32_le(sample_rate); // Sample rate
        header.put_u16(0); // Gain
        header.put_u8(0); // Channel mapping family

        audio_codec_data_tx
            .send(Some(AudioCodecData::Opus {
                ident_header: header.freeze(),
            }))
            .ok();

        let mut encoded_buffer = vec![0u8; packet_size * std::mem::size_of::<f32>()];

        let data_tx_ = data_tx.clone();
        let stream = device.build_input_stream(
            &audio_cfg.config(),
            move |mut data: &[f32], _callback_info| {
                if data_tx_.receiver_count() == 0 {
                    return;
                }

                let pts = Instant::now();

                while !data.is_empty() {
                    let to_copy = std::cmp::min(data.len(), packet_size - buffer_filled);
                    buffer[buffer_filled..buffer_filled + to_copy]
                        .copy_from_slice(&data[..to_copy]);
                    buffer_filled += to_copy;
                    data = &data[to_copy..];

                    if buffer_filled == packet_size {
                        tracing::trace!("Got a full audio packet, encoding it");
                        match encoder.encode_f32(&buffer, &mut encoded_buffer) {
                            Ok(len) => {
                                let sample =
                                    Sample::new(&encoded_buffer[..len], pts, packet_duration);
                                data_tx_.send(sample).ok();
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
            None,
        )?;
        stream.play().context("Start audio stream")?;
        tracing::info!("Audio stream started");

        current_stream = Some(stream);
    }

    Ok(())
}

pub fn setup_audio(audio_codec_data_tx: watch::Sender<Option<AudioCodecData>>) -> Result<()> {
    std::thread::spawn(move || {
        if let Err(e) = audio_thread(audio_codec_data_tx) {
            tracing::error!(?e, "Audio thread failed");
        }
    });

    Ok(())
}
