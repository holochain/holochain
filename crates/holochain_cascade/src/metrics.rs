use opentelemetry_api::{global::meter_with_version, metrics::*};
use std::sync::OnceLock;

pub type CascadeDurationMetric = Histogram<f64>;

static DURATION_METRIC: OnceLock<CascadeDurationMetric> = OnceLock::new();

pub fn create_cascade_duration_metric() -> &'static CascadeDurationMetric {
    DURATION_METRIC.get_or_init(|| {
        meter_with_version(
            "hc.cascade",
            None::<&'static str>,
            None::<&'static str>,
            Some(vec![]),
        )
        .f64_histogram("hc.cascade.duration")
        .with_unit(Unit::new("s"))
        .with_description("The time taken to execute a cascade query")
        .init()
    })
}
