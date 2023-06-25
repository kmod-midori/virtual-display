use std::{collections::HashMap, sync::Arc};

use tokio::sync::{broadcast, watch};

use crate::{audio::AudioCodecData, monitor::MonitorHandle, utils::Sample};

pub struct Application {
    pub monitors: std::sync::RwLock<HashMap<u32, MonitorHandle>>,
    pub audio_data_tx: broadcast::Sender<Sample>,
    audio_codec_data_rx: watch::Receiver<Option<AudioCodecData>>,
}

impl Application {
    pub fn new(audio_codec_data_rx: watch::Receiver<Option<AudioCodecData>>) -> Self {
        Self {
            monitors: std::sync::RwLock::new(HashMap::new()),
            audio_data_tx: tokio::sync::broadcast::channel(8).0,
            audio_codec_data_rx
        }
    }

    pub fn monitors(&self) -> std::sync::RwLockReadGuard<HashMap<u32, MonitorHandle>> {
        self.monitors.read().unwrap()
    }

    pub fn get_monitor(&self, index: u32) -> Option<MonitorHandle> {
        self.monitors().get(&index).cloned()
    }

    pub fn register_monitor(&self, index: u32, monitor: MonitorHandle) {
        self.monitors.write().unwrap().insert(index, monitor);
        tracing::info!(?index, "Registered monitor");
    }

    pub fn unregister_monitor(&self, index: u32) {
        self.monitors.write().unwrap().remove(&index);
        tracing::info!(?index, "Unregistered monitor");
    }

    pub fn audio_codec_data(&self) -> watch::Receiver<Option<AudioCodecData>> {
        self.audio_codec_data_rx.clone()
    }
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application").finish()
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationHandle(Arc<Application>);

impl ApplicationHandle {
    pub fn new(audio_codec_data_rx: watch::Receiver<Option<AudioCodecData>>) -> Self {
        Self(Arc::new(Application::new(audio_codec_data_rx)))
    }
}

impl std::ops::Deref for ApplicationHandle {
    type Target = Application;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
