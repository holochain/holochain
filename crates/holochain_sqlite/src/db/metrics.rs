use super::DbKind;
use opentelemetry::{global::meter, metrics, KeyValue};

#[derive(Clone)]
pub struct Histogram {
    histogram: metrics::Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl Histogram {
    pub fn record(&self, value: f64, _attributes: &[KeyValue]) {
        self.histogram.record(value, &self.attributes);
    }
}

pub type WriteTxnDurationMetric = Histogram;

pub fn create_write_txn_duration_metric(kind: DbKind) -> WriteTxnDurationMetric {
    let histogram = meter("hc.db")
        .f64_histogram("hc.db.write_txn.duration")
        .with_unit("s")
        .with_description("The time spent executing an exclusive write transaction")
        .build();
    let attributes = vec![
        KeyValue::new("kind", db_kind_name(kind.clone())),
        KeyValue::new("id", format!("{kind}")),
    ];
    Histogram {
        histogram,
        attributes,
    }
}

pub type ConnectionUseTimeMetric = Histogram;

pub fn create_connection_use_time_metric(kind: DbKind) -> ConnectionUseTimeMetric {
    let histogram = meter("hc.db")
        .f64_histogram("hc.db.connections.use_time")
        .with_unit("s")
        .with_description("The time between borrowing a connection and returning it to the pool")
        .build();
    let attributes = vec![
        KeyValue::new("kind", db_kind_name(kind.clone())),
        KeyValue::new("id", format!("{kind}")),
    ];
    Histogram {
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
