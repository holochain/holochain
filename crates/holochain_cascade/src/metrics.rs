use opentelemetry::global::meter;
use opentelemetry::metrics::Histogram;
use std::sync::OnceLock;

pub type CascadeDurationMetric = Histogram<f64>;

static DURATION_METRIC: OnceLock<CascadeDurationMetric> = OnceLock::new();

pub fn create_cascade_duration_metric() -> &'static CascadeDurationMetric {
    DURATION_METRIC.get_or_init(|| {
        meter("hc.cascade")
            .f64_histogram("hc.cascade.duration")
            .with_unit("s")
            .with_description("The time taken to execute a cascade query")
            .build()
    })
}
