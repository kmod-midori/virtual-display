use once_cell::sync::OnceCell;
use prometheus::{Histogram, IntCounter};

pub static METRICS: OnceCell<Metrics> = OnceCell::new();
pub fn get_metrics() -> &'static Metrics {
    METRICS.get().unwrap()
}

#[derive(Debug)]
pub struct Metrics {
    pub encoded_frames: IntCounter,
    pub end_to_end_latency_ms: Histogram,
    pub encoding_latency_ms: Histogram,
}

pub fn init() {
    let encoded_frames = IntCounter::new("encoded_frames", "Number of encoded frames").unwrap();
    let end_to_end_latency_ms = Histogram::with_opts(
        prometheus::HistogramOpts::new("end_to_end_latency_ms", "End to end latency of frames")
            .buckets(vec![3.0, 4.0, 6.0, 8.0, 10.0, 20.0, 50.0, 100.0]),
    )
    .unwrap();
    let encoding_latency_ms = Histogram::with_opts(
        prometheus::HistogramOpts::new("encoding_latency_ms", "Encoding latency of frames")
            .buckets(vec![3.0, 4.0, 6.0, 8.0, 10.0, 20.0, 50.0, 100.0]),
    )
    .unwrap();

    prometheus::register(Box::new(encoded_frames.clone())).unwrap();
    prometheus::register(Box::new(end_to_end_latency_ms.clone())).unwrap();
    prometheus::register(Box::new(encoding_latency_ms.clone())).unwrap();

    METRICS
        .set(Metrics {
            encoded_frames,
            end_to_end_latency_ms,
            encoding_latency_ms,
        })
        .unwrap();
}
