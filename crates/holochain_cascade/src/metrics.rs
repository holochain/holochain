use opentelemetry_api::{global::meter_with_version, metrics::*};

pub type CascadeDurationMetric = Histogram<f64>;

pub fn create_cascade_duration_metric() -> CascadeDurationMetric {
    meter_with_version(
        "hc.cascade",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.cascade.duration")
    .with_unit(Unit::new("s"))
    .with_description("The duration in milliseconds to execute a cascade query")
    .init()
}
