use opentelemetry::global::meter;
use opentelemetry::metrics::{Counter, Histogram};
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

pub type CascadeFetchErrorMetric = Counter<u64>;

static FETCH_ERROR_METRIC: OnceLock<CascadeFetchErrorMetric> = OnceLock::new();

pub fn cascade_fetch_error_metric() -> &'static CascadeFetchErrorMetric {
    FETCH_ERROR_METRIC.get_or_init(|| {
        meter("hc.cascade")
            .u64_counter("hc.cascade.fetch_error")
            .with_description("Number of errors encountered while fetching data from the network.")
            .build()
    })
}
