use opentelemetry_api::global::meter_with_version;
use opentelemetry_api::metrics::{Histogram, Unit};

/// A histogram metric for measuring the duration of p2p requests.
pub type P2pRequestDurationMetric = Histogram<f64>;

/// Create a new histogram metric for measuring the duration of p2p requests.
pub fn create_p2p_outgoing_request_duration_metric() -> P2pRequestDurationMetric {
    meter_with_version(
        "hc.holochain_p2p",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.holochain_p2p.request.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent sending an outgoing p2p request awaiting the response")
    .init()
}

/// Create a new histogram metric for measuring the duration of p2p requests.
pub fn create_p2p_handle_incoming_request_duration_metric() -> P2pRequestDurationMetric {
    meter_with_version(
        "hc.holochain_p2p",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.holochain_p2p.handle_request.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent handling an incoming p2p request")
    .init()
}
