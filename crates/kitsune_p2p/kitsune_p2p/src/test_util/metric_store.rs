//! An in-memory implementation of a metric store.
//! A real implementation would use a database.
// NB: this is a copy of `KdMetricStore` from `kitsune_p2p_direct`, which
//   is downstream of this crate.
//   Since we plan to delete much of these tests, I opted to keep that type
//   downstream, and just copy it upstream here for now.

use std::collections::BTreeSet;

use crate::event::*;
use kitsune_p2p_types::dependencies::observability::tracing;

/// An in-memory implementation of a metric store.
/// A real implementation would use a database.
#[derive(Default)]
pub struct KdMetricStore(BTreeSet<MetricDatum>);

impl KdMetricStore {
    /// Insert metric data into the store
    pub fn put_metric_datum(&mut self, datum: MetricDatum) {
        self.0.insert(datum);
    }

    /// Retrieve metric data from the store
    pub fn query_metrics(&self, query: MetricQuery) -> MetricQueryAnswer {
        match query {
            MetricQuery::LastSync { agent } => {
                let timestamp = self
                    .0
                    .iter()
                    .rev()
                    .find(|metric| metric.agent == agent && metric.kind == MetricKind::QuickGossip)
                    .map(|metric| metric.timestamp);
                MetricQueryAnswer::LastSync(timestamp)
            }
            MetricQuery::Oldest {
                last_connect_error_threshold,
            } => {
                tracing::warn!("This \"query\" is untested.");
                let agent = self
                    .0
                    .iter()
                    .find(|metric| {
                        metric.kind == MetricKind::ConnectError
                            && metric.timestamp <= last_connect_error_threshold
                    })
                    .map(|metric| metric.agent.clone());
                MetricQueryAnswer::Oldest(agent)
            }
        }
    }
}
