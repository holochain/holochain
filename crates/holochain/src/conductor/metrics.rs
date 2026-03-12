use opentelemetry::{global::meter, metrics, metrics::Histogram};
use std::sync::OnceLock;
use std::time::Instant;

pub(crate) type PostCommitDurationMetric = Histogram<f64>;

static POST_COMMIT_DURATION_METRIC: OnceLock<PostCommitDurationMetric> = OnceLock::new();

pub(crate) fn post_commit_duration_metric() -> &'static PostCommitDurationMetric {
    POST_COMMIT_DURATION_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .f64_histogram("hc.conductor.post_commit.duration")
            .with_unit("s")
            .with_description("The time spent executing a post commit")
            .build()
    })
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

pub(crate) type UptimeMetric = metrics::ObservableGauge<f64>;

static UPTIME_METRIC: OnceLock<UptimeMetric> = OnceLock::new();

pub(crate) fn register_uptime_metric(started_at: Instant) {
    UPTIME_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .f64_observable_gauge("hc.conductor.uptime")
            .with_unit("s")
            .with_description("The number of seconds the conductor has been running.")
            .with_callback(move |observer| {
                observer.observe(started_at.elapsed().as_secs_f64(), &[]);
            })
            .build()
    });
}
