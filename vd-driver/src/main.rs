#![allow(clippy::missing_safety_doc, clippy::new_without_default, dead_code)]

use anyhow::Result;

use monitor::Monitor;
use windows::Win32::Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL};

use std::{
    io::Read,
    time::{Duration, Instant, SystemTime}, sync::Arc,
};

use once_cell::sync::OnceCell;

mod app;
mod audio;
mod encoder;
mod metrics;
mod monitor;
mod server;
mod utils;
mod variant;
mod win32;

use app::ApplicationHandle;

use crate::win32::Waitable;

pub static TOKIO_RUNTIME: OnceCell<tokio::runtime::Runtime> = OnceCell::new();
pub fn get_tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get().unwrap()
}

pub static APPLICATION: OnceCell<ApplicationHandle> = OnceCell::new();
pub fn get_app() -> &'static ApplicationHandle {
    APPLICATION.get().unwrap()
}

#[no_mangle]
pub extern "C" fn vd_init() {
    if let Ok(log_file) = std::fs::File::create("Z:\\vd-driver.log") {
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(log_file)
            .try_init()
            .ok();
    }

    tracing::info!("vd_init");
}

fn read_configuration(buf: &[u8]) -> Option<(u32, u32, u32)> {
    let mut reader = std::io::Cursor::new(buf);

    let mut buf = [0u8; 4];

    reader.read_exact(&mut buf).ok()?;
    let configured = u32::from_le_bytes(buf);
    if configured == 0 {
        return None;
    }

    reader.read_exact(&mut buf).ok()?;
    let width = u32::from_le_bytes(buf);

    reader.read_exact(&mut buf).ok()?;
    let height = u32::from_le_bytes(buf);

    reader.read_exact(&mut buf).ok()?;
    let framerate = u32::from_le_bytes(buf);

    Some((width, height, framerate))
}

pub fn main() -> Result<()> {
    dcv_color_primitives::initialize();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    metrics::init();

    // Minimize the latency
    TOKIO_RUNTIME
        .set(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .event_interval(10)
                .global_queue_interval(10)
                .on_thread_start(|| {
                    utils::set_thread_characteristics();
                })
                .build()
                .unwrap(),
        )
        .unwrap();

    let _guard = get_tokio_runtime().enter();
    let audio_data_tx = match audio::setup_audio() {
        Ok(tx) => Some(tx),
        Err(e) => {
            tracing::error!(?e, "Failed to setup audio");
            None
        }
    };

    APPLICATION
        .set(ApplicationHandle::new(audio_data_tx))
        .unwrap();

    unsafe {
        if let Err(e) = MFStartup(
            windows::Win32::Media::MediaFoundation::MF_SDK_VERSION << 16
                | windows::Win32::Media::MediaFoundation::MF_API_VERSION,
            MFSTARTUP_FULL,
        ) {
            tracing::error!(?e, "Failed to initialize Media Foundation");
        }
    }

    server::start();

    tracing::info!("Initialized");

    let descriptor: win32::SecurityDescriptor = "D:(A;;0xc01f0003;;;AU)".parse()?;

    let frame_buffer_mutex = win32::Mutex::new("Global\\VdMonitor0FBMutex", Some(&descriptor))?;

    let new_frame_event = win32::Event::new(
        "Global\\VdMonitor0NewFrameEvent",
        Some(&descriptor),
        false,
        false,
    )?;

    let configure_event = win32::Event::new(
        "Global\\VdMonitor0ConfigureEvent",
        Some(&descriptor),
        false,
        false,
    )?;

    let (mut frame_buffer_mapping, map_already_exists) = unsafe {
        win32::FileMapping::new("Global\\VdMonitor0FB", Some(&descriptor), 1024 * 1024 * 20)?
    };

    if !map_already_exists {
        // We created the map, so we need to initialize it
        // let _guard = frame_buffer_mutex.lock()?;
        frame_buffer_mapping.buf_mut()[0..4].copy_from_slice(&[0, 0, 0, 0]);
    }

    let cursor_buffer_mutex =
        win32::Mutex::new("Global\\VdMonitor0CursorMutex", Some(&descriptor))?;

    let cursor_position_event = win32::Event::new(
        "Global\\VdMonitor0CursorPositionUpdatedEvent",
        Some(&descriptor),
        false,
        false,
    )?;

    let cursor_image_event = win32::Event::new(
        "Global\\VdMonitor0CursorImageUpdatedEvent",
        Some(&descriptor),
        false,
        false,
    )?;

    let (cursor_mapping, _) = unsafe {
        win32::FileMapping::new(
            "Global\\VdMonitor0Cursor",
            Some(&descriptor),
            1024 * 128 + 4 * 6,
        )?
    };

    tracing::info!("Running");

    let monitor = Arc::new(Monitor::new(0));

    let initial_configuration = {
        let _guard = frame_buffer_mutex.lock()?;
        read_configuration(frame_buffer_mapping.buf())
    };

    let initial_configuration = if let Some(cfg) = initial_configuration {
        cfg
    } else {
        tracing::info!("Waiting for initial configuration");
        configure_event.wait(None)?;
        let _guard = frame_buffer_mutex.lock()?;
        read_configuration(frame_buffer_mapping.buf()).expect("Failed to read configuration")
    };

    monitor.configure(
        initial_configuration.0,
        initial_configuration.1,
        initial_configuration.2,
    );

    // Send frames to the monitor
    let monitor_ = monitor.clone();
    tokio::spawn(async move {
        let _ = new_frame_event.wait(None); // Sync with the first available frame

        let mut ticker = tokio::time::interval(Duration::from_millis(16));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let guard = frame_buffer_mutex.lock().unwrap();
            monitor_.send_frame(
                &frame_buffer_mapping.buf()[..1920 * 1080 * 4],
                Instant::now(),
            );
            drop(guard);

            let _ = ticker.tick().await;
        }
    });

    loop {
        let w = win32::wait_multiple(&[&cursor_image_event, &cursor_position_event], None)?;

        match w {
            win32::WaitState::Signaled(0) | win32::WaitState::Abandoned(0) => {
                tracing::info!("Cursor image updated");
                let _guard = cursor_buffer_mutex.lock()?;

                let buf = cursor_mapping.buf();
                
                let width = u32::from_ne_bytes(buf[12..16].try_into().unwrap());
                let height = u32::from_ne_bytes(buf[16..20].try_into().unwrap());
                let pitch = u32::from_ne_bytes(buf[20..24].try_into().unwrap());

                dbg!(width, height, pitch);
            }
            win32::WaitState::Signaled(1) | win32::WaitState::Abandoned(1) => {
                let buf = cursor_mapping.buf();

                // Coordinates might be negative, so we need to use i32
                let x = i32::from_ne_bytes(buf[0..4].try_into().unwrap());
                let y = i32::from_ne_bytes(buf[4..8].try_into().unwrap());
                let visible = u32::from_ne_bytes(buf[8..12].try_into().unwrap()) == 1;
                
                monitor.set_cursor_position(x, y, visible);
            }
            _ => unreachable!(),
        }
    }
}
