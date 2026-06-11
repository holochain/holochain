use opentelemetry::global::meter;
use opentelemetry::metrics::Counter;
use std::sync::OnceLock;

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
