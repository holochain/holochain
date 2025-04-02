use opentelemetry_api::global::meter_with_version;
use opentelemetry_api::metrics::{Histogram, Unit};

/// A histogram metric for measuring the duration of p2p requests.
pub type P2pRequestDurationMetric = Histogram<f64>;

/// Create a new histogram metric for measuring the duration of p2p requests.
pub fn create_p2p_request_duration_metric() -> P2pRequestDurationMetric {
    meter_with_version(
        "hc.holochain_p2p",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.holochain_p2p.request.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent processing a p2p event")
    .init()
}
