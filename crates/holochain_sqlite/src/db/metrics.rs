use super::DbKind;
use opentelemetry::{global::meter, metrics::Histogram, KeyValue};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub fn create_pool_usage_metric(kind: DbKind, db_semaphores: Vec<Arc<Semaphore>>) {
    let total_permits: usize = db_semaphores.iter().map(|s| s.available_permits()).sum();
    meter("hc.db")
        .f64_observable_gauge("hc.db.pool.utilization")
        .with_description("The utilisation of connections in the pool")
        .with_callback(move |observer| {
            let current_permits: usize = db_semaphores.iter().map(|s| s.available_permits()).sum();

            observer.observe(
                (total_permits - current_permits) as f64 / total_permits as f64,
                &[
                    KeyValue::new("kind", db_kind_name(kind.clone())),
                    KeyValue::new("id", format!("{kind}")),
                ],
            )
        })
        .build();
}

#[derive(Clone)]
pub struct UseTimeMetric {
    histogram: Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl UseTimeMetric {
    pub fn record(&self, value: f64, _attributes: &[KeyValue]) {
        self.histogram.record(value, &self.attributes);
    }
}

pub fn create_connection_use_time_metric(kind: DbKind) -> UseTimeMetric {
    let histogram = meter("hc.db")
        .f64_histogram("hc.db.connections.use_time")
        .with_unit("s")
        .with_description("The time between borrowing a connection and returning it to the pool")
        .build();
    let attributes = vec![
        KeyValue::new("kind", db_kind_name(kind.clone())),
        KeyValue::new("id", format!("{kind}")),
    ];
    UseTimeMetric {
        histogram,
        attributes,
    }
}

fn db_kind_name(kind: DbKind) -> String {
    match kind {
        DbKind::Authored(_) => "authored",
        DbKind::Dht(_) => "dht",
        DbKind::Cache(_) => "cache",
        DbKind::Conductor => "conductor",
        DbKind::Wasm => "wasm",
        DbKind::PeerMetaStore(_) => "peer_meta_store",
        #[cfg(feature = "test_utils")]
        DbKind::Test(_) => "test",
    }
    .to_string()
}
