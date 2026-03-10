use opentelemetry::{global::meter, metrics, metrics::Histogram};
use std::sync::OnceLock;

pub type PostCommitDurationMetric = Histogram<f64>;

pub fn create_post_commit_duration_metric() -> PostCommitDurationMetric {
    meter("hc.conductor")
        .f64_histogram("hc.conductor.post_commit.duration")
        .with_unit("s")
        .with_description("The time spent executing a post commit")
        .build()
}

pub(crate) type DroppedSignalMetric = metrics::Counter<u64>;

static DROPPED_SIGNAL_METRIC: OnceLock<DroppedSignalMetric> = OnceLock::new();

pub(crate) fn dropped_signal_metric() -> &'static DroppedSignalMetric {
    DROPPED_SIGNAL_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .u64_counter("hc.conductor.app_ws.dropped_signal")
            .with_description("The number of signals dropped from app ws due to channel overload.")
            .build()
    })
}
