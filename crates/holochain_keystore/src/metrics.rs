use opentelemetry::global::meter;
use opentelemetry::metrics::Histogram;
use std::sync::OnceLock;

pub type LairRequestDurationMetric = Histogram<f64>;

static LAIR_REQUEST_DURATION_METRIC: OnceLock<LairRequestDurationMetric> = OnceLock::new();

pub fn lair_request_duration_metric() -> &'static LairRequestDurationMetric {
    LAIR_REQUEST_DURATION_METRIC.get_or_init(|| {
        meter("hc.keystore")
            .f64_histogram("hc.keystore.lair_request.duration")
            .with_unit("s")
            .with_description("Duration of signing and encryption requests to Lair.")
            .build()
    })
}
