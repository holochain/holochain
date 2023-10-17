pub type CascadeDurationMetric = opentelemetry_api::metrics::Histogram<u64>;

pub fn create_cascade_duration_metric() -> CascadeDurationMetric {
    opentelemetry_api::global::meter_with_version(
        "hc.cascade",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .u64_histogram("hc.cascade.duration")
    .with_unit(opentelemetry_api::metrics::Unit::new("ms"))
    .with_description("The duration in milliseconds to execute a cascade query")
    .init()
}
