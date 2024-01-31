use super::DbKind;
use opentelemetry_api::{global::meter_with_version, metrics::*, KeyValue};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub type UseTimeMetric = Histogram<f64>;

pub fn create_pool_usage_metric(kind: DbKind, db_semaphores: Vec<Arc<Semaphore>>) {
    let meter = meter_with_version(
        "hc.db",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![
            KeyValue::new("kind", db_kind_name(kind.clone())),
            KeyValue::new("id", format!("{}", kind)),
        ]),
    );

    let gauge = meter
        .f64_observable_gauge("hc.db.pool.utilization")
        .with_description("The utilisation of connections in the pool")
        .init();

    let total_permits: usize = db_semaphores.iter().map(|s| s.available_permits()).sum();
    match meter.register_callback(&[gauge.as_any()], move |observer| {
        let current_permits: usize = db_semaphores.iter().map(|s| s.available_permits()).sum();

        observer.observe_f64(
            &gauge,
            (total_permits - current_permits) as f64 / total_permits as f64,
            &[],
        )
    }) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to register callback for metric: {:?}", e);
        }
    };
}

pub fn create_connection_use_time_metric(kind: DbKind) -> UseTimeMetric {
    meter_with_version(
        "hc.db",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![
            KeyValue::new("kind", db_kind_name(kind.clone())),
            KeyValue::new("id", format!("{}", kind)),
        ]),
    )
    .f64_histogram("hc.db.connections.use_time")
    .with_unit(Unit::new("s"))
    .with_description("The time between borrowing a connection and returning it to the pool")
    .init()
}

fn db_kind_name(kind: DbKind) -> String {
    match kind {
        DbKind::Authored(_) => "authored",
        DbKind::Dht(_) => "dht",
        DbKind::Cache(_) => "cache",
        DbKind::Conductor => "conductor",
        DbKind::Wasm => "wasm",
        DbKind::P2pAgentStore(_) => "p2p_agent_store",
        DbKind::P2pMetrics(_) => "p2p_metrics",
        #[cfg(feature = "test_utils")]
        DbKind::Test(_) => "test",
    }
    .to_string()
}
