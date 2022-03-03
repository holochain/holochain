//! Implementation of the Kitsune Host API

use std::sync::Arc;

use futures::FutureExt;
use holo_hash::DnaHash;
use holochain_p2p::DnaHashExt;
use kitsune_p2p::{KitsuneHost, KitsuneHostResult};

use super::space::Spaces;
use holochain_types::env::PermittedConn;

/// Implementation of the Kitsune Host API.
/// Lets Kitsune make requests of Holochain
pub struct KitsuneHostImpl {
    spaces: Spaces,
}

impl KitsuneHostImpl {
    /// Constructor
    pub fn new(spaces: Spaces) -> Arc<Self> {
        Arc::new(Self { spaces })
    }
}

impl KitsuneHost for KitsuneHostImpl {
    fn peer_extrapolated_coverage(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc_set: holochain_p2p::dht_arc::DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        async move {
            let env = self.spaces.p2p_env(&DnaHash::from_kitsune(&space))?;
            use holochain_sqlite::db::AsP2pAgentStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_extrapolated_coverage(dht_arc_set)
            })
            .await;
            let res = res.map_err(Box::new)?.map_err(Box::new)?;
            Ok(res)
        }
        .boxed()
        .into()
    }

    fn record_metrics(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        records: Vec<kitsune_p2p::event::MetricRecord>,
    ) -> KitsuneHostResult<()> {
        async move {
            let env = self
                .spaces
                .p2p_metrics_env(&DnaHash::from_kitsune(&space))?;
            use holochain_sqlite::db::AsP2pMetricStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_log_metrics(records)
            })
            .await;
            let res = res.map_err(Box::new)?.map_err(Box::new)?;
            Ok(res)
        }
        .boxed()
        .into()
    }
}
