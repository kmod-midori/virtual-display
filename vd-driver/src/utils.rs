#[derive(Debug, Clone)]
pub struct Sample {
    pub data: Arc<Vec<u8>>,
    pub timestamp: SystemTime,
    pub duration: Duration,
}

impl Sample {
    pub fn new(data: impl AsRef<[u8]>, timestamp: SystemTime, duration: Duration) -> Self {
        Self {
            data: Arc::new(data.as_ref().to_vec()),
            timestamp,
            duration,
        }
    }

    pub fn record_end_to_end_latency(&self) {
        let end_to_end_latency = &crate::metrics::get_metrics().end_to_end_latency_ms;
        if let Ok(dur) = self.timestamp.elapsed() {
            end_to_end_latency.observe(dur.as_secs_f64() * 1000.0);
        }
    }
}


/// Set the thread characteristics to notify the system that this thread is
/// a high priority thread.
pub fn set_thread_characteristics() {
    let mut task_index = 0;
    let res = unsafe {
        windows::Win32::System::Threading::AvSetMmThreadCharacteristicsW(
            windows::w!("Distribution"),
            &mut task_index,
        )
    };

    if let Err(e) = res {
        tracing::error!(?e, "Failed to set thread characteristics");
    }
}

use std::{time::{SystemTime, Duration}, sync::Arc};

use dcv_color_primitives as dcp;

pub fn bgra2nv12(
    width: u32,
    height: u32,
    src: &[u8],
    dst_stride: Option<usize>,
    dst_y: &mut [u8],
    dst_uv: &mut [u8],
) -> Result<(), dcp::ErrorKind> {
    let dcp_src_format = dcp::ImageFormat {
        pixel_format: dcp::PixelFormat::Bgra,
        color_space: dcp::ColorSpace::Rgb,
        num_planes: 1,
    };

    let dcp_dst_format = dcp::ImageFormat {
        pixel_format: dcp::PixelFormat::Nv12,
        color_space: dcp::ColorSpace::Bt709,
        num_planes: 2,
    };

    let dst_stride = dst_stride.map(|x| [x, x]);

    dcp::convert_image(
        width,
        height,
        &dcp_src_format,
        None,
        &[src],
        &dcp_dst_format,
        dst_stride.as_ref().map(|x| &x[..]),
        &mut [dst_y, dst_uv],
    )?;

    Ok(())
}