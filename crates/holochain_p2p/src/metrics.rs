use opentelemetry::global::meter;
use opentelemetry::metrics;
use opentelemetry::metrics::{Counter, Histogram};

/// A histogram metric for measuring the duration of p2p requests.
pub type P2pRequestDurationMetric = Histogram<f64>;

/// A counter metric for measuring the number of incoming p2p requests that have been ignored.
pub type P2pRequestIgnoredMetric = Counter<u64>;

/// Create a new histogram metric for measuring the duration of p2p requests.
pub fn create_p2p_outgoing_request_duration_metric() -> P2pRequestDurationMetric {
    meter("hc.holochain_p2p")
        .f64_histogram("hc.holochain_p2p.request.duration")
        .with_unit("s")
        .with_description("The time spent sending an outgoing p2p request awaiting the response")
        .build()
}

/// Create a new histogram metric for measuring the duration of p2p requests.
pub fn create_p2p_handle_incoming_request_duration_metric() -> P2pRequestDurationMetric {
    meter("hc.holochain_p2p")
        .f64_histogram("hc.holochain_p2p.handle_request.duration")
        .with_unit("s")
        .with_description("The time spent handling an incoming p2p request")
        .build()
}

/// Create a new counter metric for counting ignored p2p requests.
pub fn create_p2p_handle_incoming_request_ignored_metric() -> P2pRequestIgnoredMetric {
    meter("hc.holochain_p2p")
        .u64_counter("hc.holochain_p2p.handle_request.ignored")
        .with_unit("requests")
        .with_description("The number of incoming p2p requests that have been ignored.")
        .build()
}

/// A counter metric for counting received remote signals.
pub type P2pRecvRemoteSignalMetric = metrics::Counter<u64>;

/// Create a new counter metric for counting received remote signals.
pub fn create_p2p_recv_remote_signal_metric() -> P2pRecvRemoteSignalMetric {
    meter("hc.holochain_p2p")
        .u64_counter("hc.holochain_p2p.recv_remote_signal.count")
        .with_description("The number of remote signals received.")
        .build()
}
