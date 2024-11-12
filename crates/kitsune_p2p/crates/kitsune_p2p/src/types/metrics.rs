#![allow(dead_code)]

use crate::types::metrics_helper;
use ghost_actor::dependencies::tracing;

metrics_helper::metrics!(
    KitsuneMetrics,
    Failure,
    Call,
    CallResp,
    Notify,
    NotifyResp,
    Gossip,
    PeerGet,
    PeerGetResp,
    PeerQuery,
    PeerQueryResp
);

/// Print all metrics as tracing events
#[cfg_attr(feature = "instrument", tracing::instrument)]
#[allow(dead_code)]
pub fn print_all_metrics() {
    if metrics_helper::is_enabled() {
        use std::fmt::Write;
        use KitsuneMetrics::*;
        let mut out = String::new();
        writeln!(
            out,
            "\n**************************\n* Kitsune Metrics Report *\n**************************\n",
        )
        .expect("Failed to print metrics");
        for (metric, count) in KitsuneMetrics::iter() {
            match metric {
                Call | Notify | Gossip | PeerGet | PeerQuery => {
                    writeln!(
                        out,
                        "metric: {:?} {}Bytes {:.4}MB",
                        metric,
                        count,
                        count as f64 / 1_000_000.0,
                    )
                    .expect("Failed to print metrics");
                }
                Failure | CallResp | NotifyResp | PeerGetResp | PeerQueryResp => {
                    writeln!(
                        out,
                        "metric: {:?} {}Bytes {:.4}MB",
                        metric,
                        count,
                        count as f64 / 1_000_000.0,
                    )
                    .expect("Failed to print metrics");
                }
            }
        }
        tracing::trace!(metric = %out);
    }
}

/// Turn on metrics if `KITSUNE_METRICS=ON`
pub fn init() {
    if let Some(km) = std::env::var_os("KITSUNE_METRICS") {
        if km == "ON" {
            metrics_helper::init();
        }
    }
}
