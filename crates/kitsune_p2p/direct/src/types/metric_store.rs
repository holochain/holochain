//! An in-memory implementation of a metric store.
//! A real implementation would use a database.

use std::collections::BTreeSet;

use kitsune_p2p::event::*;
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
