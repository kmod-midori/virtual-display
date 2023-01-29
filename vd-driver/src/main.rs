#![allow(clippy::missing_safety_doc, clippy::new_without_default, dead_code)]

use anyhow::Result;

use monitor::Monitor;
use windows::Win32::Media::MediaFoundation::{MFStartup, MFSTARTUP_FULL};

use std::{io::Read, time::SystemTime};

use once_cell::sync::OnceCell;

mod app;
mod audio;
mod encoder;
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

#[no_mangle]
pub unsafe extern "C" fn vd_monitor_send_frame(
    monitor: *mut Monitor,
    in_buffer: *const u8,
    len: usize,
) {
    let monitor = &mut *monitor;
    let buffer = std::slice::from_raw_parts(in_buffer, len);
    monitor.send_frame(buffer, SystemTime::now());
}

#[no_mangle]
pub unsafe extern "C" fn vd_monitor_configure(
    monitor: *mut Monitor,
    width: u32,
    height: u32,
    framerate: u32,
) {
    let monitor = unsafe { &mut *monitor };
    monitor.configure(width, height, framerate);
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
    // let audio_data_tx = None;

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

    // let frame_buffer_mutex = win32::Mutex::new("Global\\VdMonitor0FBMutex", Some(&descriptor))?;

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

    tracing::info!("Running");

    let mut monitor = Monitor::new(0);

    let initial_configuration = {
        // let _guard = frame_buffer_mutex.lock()?;
        read_configuration(frame_buffer_mapping.buf())
    };

    let initial_configuration = if let Some(cfg) = initial_configuration {
        cfg
    } else {
        tracing::info!("Waiting for initial configuration");
        configure_event.wait(None)?;
        // let _guard = frame_buffer_mutex.lock()?;
        read_configuration(frame_buffer_mapping.buf()).expect("Failed to read configuration")
    };

    monitor.configure(
        initial_configuration.0,
        initial_configuration.1,
        initial_configuration.2,
    );

    loop {
        let w = win32::wait_multiple(&[&new_frame_event, &configure_event], None)?;

        match w {
            win32::WaitState::Signaled(0) | win32::WaitState::Abandoned(0) => {
                // let _guard = frame_buffer_mutex.lock()?;
                monitor.send_frame(
                    &frame_buffer_mapping.buf()[..1920 * 1080 * 4],
                    SystemTime::now(),
                );
            }
            win32::WaitState::Signaled(1) | win32::WaitState::Abandoned(1) => {
                tracing::info!("Configure");
            }
            _ => unreachable!(),
        }
    }
}
