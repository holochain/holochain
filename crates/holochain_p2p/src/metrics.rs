use opentelemetry::global::meter;
use opentelemetry::metrics;
use opentelemetry::metrics::{Counter, Histogram};
use std::sync::OnceLock;

/// A histogram metric for measuring the duration of p2p requests.
pub type P2pRequestDurationMetric = Histogram<f64>;

/// A counter metric for measuring the number of incoming p2p requests that have been ignored.
pub type P2pRequestIgnoredMetric = Counter<u64>;

static P2P_OUTGOING_REQUEST_DURATION_METRIC: OnceLock<P2pRequestDurationMetric> = OnceLock::new();

/// Metric for measuring the duration of outgoing p2p requests.
pub fn p2p_outgoing_request_duration_metric() -> &'static P2pRequestDurationMetric {
    P2P_OUTGOING_REQUEST_DURATION_METRIC.get_or_init(|| {
        meter("hc.holochain_p2p")
            .f64_histogram("hc.holochain_p2p.request.duration")
            .with_unit("s")
            .with_description("The time spent sending an outgoing p2p request awaiting the response")
            .build()
    })
}

static P2P_HANDLE_INCOMING_REQUEST_DURATION_METRIC: OnceLock<P2pRequestDurationMetric> =
    OnceLock::new();

/// Metric for measuring the duration of handling incoming p2p requests.
pub fn p2p_handle_incoming_request_duration_metric() -> &'static P2pRequestDurationMetric {
    P2P_HANDLE_INCOMING_REQUEST_DURATION_METRIC.get_or_init(|| {
        meter("hc.holochain_p2p")
            .f64_histogram("hc.holochain_p2p.handle_request.duration")
            .with_unit("s")
            .with_description("The time spent handling an incoming p2p request")
            .build()
    })
}

static P2P_HANDLE_INCOMING_REQUEST_IGNORED_METRIC: OnceLock<P2pRequestIgnoredMetric> =
    OnceLock::new();

/// Metric for counting ignored incoming p2p requests.
pub fn p2p_handle_incoming_request_ignored_metric() -> &'static P2pRequestIgnoredMetric {
    P2P_HANDLE_INCOMING_REQUEST_IGNORED_METRIC.get_or_init(|| {
        meter("hc.holochain_p2p")
            .u64_counter("hc.holochain_p2p.handle_request.ignored")
            .with_unit("requests")
            .with_description("The number of incoming p2p requests that have been ignored.")
            .build()
    })
}

/// A counter metric for counting received remote signals.
pub type P2pRecvRemoteSignalMetric = metrics::Counter<u64>;

static P2P_RECV_REMOTE_SIGNAL_METRIC: OnceLock<P2pRecvRemoteSignalMetric> = OnceLock::new();

/// Metric for counting received remote signals.
pub fn p2p_recv_remote_signal_metric() -> &'static P2pRecvRemoteSignalMetric {
    P2P_RECV_REMOTE_SIGNAL_METRIC.get_or_init(|| {
        meter("hc.holochain_p2p")
            .u64_counter("hc.holochain_p2p.recv_remote_signal")
            .with_description("The number of remote signals received.")
            .build()
    })
}
