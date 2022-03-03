//! Implementation of the Kitsune Host API

use std::sync::Arc;

use futures::FutureExt;
use holo_hash::DnaHash;
use holochain_p2p::{dht::quantum::Topology, DnaHashExt};
use kitsune_p2p::{
    agent_store::AgentInfoSigned, event::GetAgentInfoSignedEvt, KitsuneHost, KitsuneHostResult,
};

use super::space::Spaces;
use holochain_types::{env::PermittedConn, prelude::DnaStore, share::RwShare};

/// Implementation of the Kitsune Host API.
/// Lets Kitsune make requests of Holochain
pub struct KitsuneHostImpl {
    spaces: Spaces,
    dna_store: RwShare<DnaStore>,
}

impl KitsuneHostImpl {
    /// Constructor
    pub fn new(spaces: Spaces, dna_store: RwShare<DnaStore>) -> Arc<Self> {
        Arc::new(Self { spaces, dna_store })
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
            let task = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_extrapolated_coverage(dht_arc_set)
            })
            .await;
            Ok(task??)
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
            let task = tokio::task::spawn_blocking(move || {
                let mut conn = env.from_permit(permit)?;
                conn.p2p_log_metrics(records)
            })
            .await;
            Ok(task??)
        }
        .boxed()
        .into()
    }

    fn get_agent_info_signed(
        &self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<AgentInfoSigned>> {
        let dna_hash = DnaHash::from_kitsune(&space);
        let env = self.spaces.p2p_env(&dna_hash);
        async move {
            Ok(super::p2p_agent_store::get_agent_info_signed(env?.into(), space, agent).await?)
        }
        .boxed()
        .into()
    }

    fn query_region_set(
        &self,
        space: &kitsune_p2p::KitsuneSpace,
        dht_arc_set: Arc<holochain_p2p::dht_arc::DhtArcSet>,
    ) -> KitsuneHostResult<holochain_p2p::dht::region::RegionSetXtcs> {
        let dna_hash = DnaHash::from_kitsune(&Arc::new(space.clone()));
        async move {
            Ok(self
                .spaces
                .handle_fetch_op_regions(dna_hash, dht_arc_set)
                .await?)
        }
        .boxed()
        .into()
    }

    fn get_topology(&self, space: Arc<kitsune_p2p::KitsuneSpace>) -> Option<Topology> {
        let dna_hash = DnaHash::from_kitsune(&Arc::new(space.clone()));
        let dna_def = self.dna_store.get_dna_def(&dna_hash)?;
        Some(Topology::standard(dna_def.origin_time))
    }
}
