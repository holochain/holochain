use ghost_actor::dependencies::tracing;

observability::metrics!(
    KitsuneMetrics,
    Call,
    CallResp,
    Notify,
    NotifyResp,
    FetchOpHashes,
    FetchOpHashesResp,
    FetchOpData,
    FetchOpDataResp,
    AgentInfoQuery,
    AgentInfoQueryResp,
    Gossip,
    GossipResp,
    Fail
);

/// Print all metrics as tracing events
#[tracing::instrument]
pub fn print_all_metrics() {
    if observability::metrics::is_enabled() {
        use kitsune_p2p_types::transport::KitsuneTransportMetrics;
        use std::fmt::Write;
        let writes = KitsuneTransportMetrics::get(KitsuneTransportMetrics::Write);
        let reads = KitsuneTransportMetrics::get(KitsuneTransportMetrics::Read);
        let total_writes = writes as f64;
        let total_reads = reads as f64;
        use KitsuneMetrics::*;
        let mut out = String::new();
        writeln!(
            out,
            "\n**************************\n* Kitsune Metrics Report *\n**************************\n",
        )
        .expect("Failed to print metrics");
        for (metric, count) in KitsuneMetrics::iter() {
            match metric {
                Call | Notify | FetchOpHashes | FetchOpData | AgentInfoQuery | Gossip => {
                    let percent = if total_writes > 0.0 {
                        count as f64 / total_writes * 100.0
                    } else {
                        0.0
                    };
                    writeln!(
                        out,
                        "metric: {:?} {}Bytes {:.4}MB percent_of_writes: {:.2}%",
                        metric,
                        count,
                        count as f64 / 1_000_000.0,
                        percent
                    )
                    .expect("Failed to print metrics");
                }
                CallResp | NotifyResp | FetchOpHashesResp | FetchOpDataResp
                | AgentInfoQueryResp | GossipResp | Fail => {
                    let percent = if total_reads > 0.0 {
                        count as f64 / total_reads * 100.0
                    } else {
                        0.0
                    };
                    writeln!(
                        out,
                        "metric: {:?} {}Bytes {:.4}MB percent_of_reads: {:.2}%",
                        metric,
                        count,
                        count as f64 / 1_000_000.0,
                        percent
                    )
                    .expect("Failed to print metrics");
                }
            }
        }
        writeln!(
            out,
            "total writes: {}Bytes {:.4}MB total reads: {}Bytes {:.4}MB",
            writes,
            writes as f64 / 1_000_000.0,
            reads,
            reads as f64 / 1_000_000.0,
        )
        .expect("Failed to print metrics");
        tracing::trace!(metric = %out);
    }
}

/// Turn on metrics if `KITSUNE_METRICS=ON`
pub fn init() {
    if let Some(km) = std::env::var_os("KITSUNE_METRICS") {
        if km == "ON" {
            observability::metrics::init();
        }
    }
}
