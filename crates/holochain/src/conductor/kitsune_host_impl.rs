//! Implementation of the Kitsune Host API

use futures::FutureExt;
use kitsune_p2p::{KitsuneHost, KitsuneHostResult};

use super::ConductorHandle;
use holochain_types::env::PermittedConn;

/// Implementation of the Kitsune Host API.
/// Lets Kitsune make requests of Holochain
pub struct KitsuneHostImpl {
    conductor: ConductorHandle,
}

impl KitsuneHost for KitsuneHostImpl {
    fn peer_extrapolated_coverage(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc_set: holochain_p2p::dht_arc::DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        let env = self.conductor.get_p2p_env(space);
        async move {
            use holochain_sqlite::db::AsP2pAgentStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_extrapolated_coverage(dht_arc_set)
            })
            .await;
            let res = res
                .map_err(holochain_p2p::HolochainP2pError::other)
                .and_then(|r| r.map_err(holochain_p2p::HolochainP2pError::other))?;
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
        let env = self.conductor.get_p2p_metrics_env(space);
        async move {
            use holochain_sqlite::db::AsP2pMetricStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_log_metrics(records)
            })
            .await;
            let res = res
                .map_err(holochain_p2p::HolochainP2pError::other)
                .and_then(|r| r.map_err(holochain_p2p::HolochainP2pError::other))?;
            Ok(res)
        }
        .boxed()
        .into()
    }
}

/*
holochain_p2p::event::HolochainP2pEvent::KGenReq { arg, respond, .. } => match arg {
    KGenReq::PeerExtrapCov { space, dht_arc_set } => {
        let env = { self.p2p_env(space) };
        respond.respond(Ok(async move {
            use holochain_sqlite::db::AsP2pAgentStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_extrapolated_coverage(dht_arc_set)
            })
            .await;
            let res = res
                .map_err(holochain_p2p::HolochainP2pError::other)
                .and_then(|r| r.map_err(holochain_p2p::HolochainP2pError::other))?;
            Ok(KGenRes::PeerExtrapCov(res))
        }
        .boxed()
        .into()));
    }
    KGenReq::RecordMetrics { space, records } => {
        let env = { self.p2p_metrics_env(space) };
        respond.respond(Ok(async move {
            use holochain_sqlite::db::AsP2pMetricStoreConExt;
            let permit = env.conn_permit().await;
            let res = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_log_metrics(records)
            })
            .await;
            let res = res
                .map_err(holochain_p2p::HolochainP2pError::other)
                .and_then(|r| r.map_err(holochain_p2p::HolochainP2pError::other))?;
            Ok(KGenRes::RecordMetrics(res))
        }
        .boxed()
        .into()));
    }
},
*/
