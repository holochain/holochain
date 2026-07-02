//! OpenTelemetry instrumentation for the database layer.
//!
//! Emits the `hc.db.connections.use_time` metric for reads served by the
//! `holochain_data` store. The metric is created once per database handle and
//! recorded each time a borrowed connection is returned to the pool (see
//! [`crate::handles::TimedConn`]).

use crate::kind::DbKind;
use crate::DatabaseIdentifier;
use opentelemetry::{global::meter, metrics, KeyValue};

/// An OpenTelemetry `f64` histogram pre-bound to a fixed set of attributes.
///
/// Cloning is cheap; clones share the same underlying instrument and
/// attribute set, so a handle can hand a clone to every connection guard it
/// creates.
#[derive(Clone)]
pub(crate) struct Histogram {
    histogram: metrics::Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl std::fmt::Debug for Histogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `metrics::Histogram` does not implement `Debug`, so report only the
        // bound attributes (which is the useful part for diagnostics).
        f.debug_struct("Histogram")
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

impl Histogram {
    /// Record a value against the pre-bound attributes.
    pub(crate) fn record(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }
}

/// Metric for `hc.db.connections.use_time`.
pub(crate) type ConnectionUseTimeMetric = Histogram;

/// Create a [`ConnectionUseTimeMetric`] bound to the given database identifier.
///
/// The histogram carries two attributes derived from `identifier`:
/// - `kind`: the database kind (e.g. `dht`), and
/// - `id`: the stable per-database identifier ([`DatabaseIdentifier::database_id`]).
pub(crate) fn create_connection_use_time_metric<I: DatabaseIdentifier>(
    identifier: &I,
) -> ConnectionUseTimeMetric {
    let histogram = meter("hc.db")
        .f64_histogram("hc.db.connections.use_time")
        .with_unit("s")
        .with_description("The time between borrowing a connection and returning it to the pool")
        .build();
    let attributes = vec![
        KeyValue::new("kind", db_kind_name(identifier.db_kind())),
        KeyValue::new("id", identifier.database_id().to_string()),
    ];
    Histogram {
        histogram,
        attributes,
    }
}

/// Stable metric label for a database kind.
fn db_kind_name(kind: DbKind) -> &'static str {
    match kind {
        DbKind::Wasm => "wasm",
        DbKind::Conductor => "conductor",
        DbKind::PeerMetaStore => "peer_meta_store",
        DbKind::Dht => "dht",
    }
}
