use opentelemetry_api::global::meter_with_version;
use opentelemetry_api::metrics::{Counter, Histogram, Unit};

/// A histogram metric for measuring the duration of p2p requests.
pub type P2pRequestDurationMetric = Histogram<f64>;

/// A counter metric for measuring the number of incoming p2p requests that have been ignored.
pub type P2pRequestIgnoredMetric = Counter<u64>;

/// Create a new histogram metric for measuring the duration of outgoing p2p requests,
/// up until they are handed off to the transport.
pub fn create_p2p_outgoing_request_duration_metric() -> P2pRequestDurationMetric {
    meter_with_version(
        "hc.holochain_p2p",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.holochain_p2p.request.duration")
    .with_unit(Unit::new("s"))
    .with_description(
        "The time spent sending an outgoing p2p request until it is handed off to the transport",
    )
    .init()
}

/// Create a new histogram metric for measuring the duration of handling incoming p2p requests.
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

/// Create a new counter metric for counting ignored p2p requests.
pub fn create_p2p_handle_incoming_request_ignored_metric() -> P2pRequestIgnoredMetric {
    meter_with_version(
        "hc.holochain_p2p",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .u64_counter("hc.holochain_p2p.handle_request.ignored")
    .with_unit(Unit::new("requests"))
    .with_description("The number of incoming p2p requests that have been ignored.")
    .init()
}
