//! Implementation of the Kitsune Host API

mod query_region_set;
mod query_size_limited_regions;

use std::sync::Arc;

use super::{ribosome_store::RibosomeStore, space::Spaces};
use futures::FutureExt;
use holo_hash::DnaHash;
use holochain_p2p::{
    dht::{spacetime::Topology, ArqStrat},
    DnaHashExt,
};
use holochain_types::{
    db::PermittedConn,
    prelude::{DhtOpHash, DnaError},
    share::RwShare,
};
use kitsune_p2p::{
    agent_store::AgentInfoSigned, event::GetAgentInfoSignedEvt, KitsuneHost, KitsuneHostResult,
};
use kitsune_p2p_types::{config::KitsuneP2pTuningParams, KOpData, KOpHash};

/// Implementation of the Kitsune Host API.
/// Lets Kitsune make requests of Holochain
pub struct KitsuneHostImpl {
    spaces: Spaces,
    ribosome_store: RwShare<RibosomeStore>,
    tuning_params: KitsuneP2pTuningParams,
    strat: ArqStrat,
}

impl KitsuneHostImpl {
    /// Constructor
    pub fn new(
        spaces: Spaces,
        ribosome_store: RwShare<RibosomeStore>,
        tuning_params: KitsuneP2pTuningParams,
        strat: ArqStrat,
    ) -> Arc<Self> {
        Arc::new(Self {
            spaces,
            ribosome_store,
            tuning_params,
            strat,
        })
    }
}

impl KitsuneHost for KitsuneHostImpl {
    fn peer_extrapolated_coverage(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc_set: holochain_p2p::dht_arc::DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        async move {
            let db = self.spaces.p2p_agents_db(&DnaHash::from_kitsune(&space))?;
            use holochain_sqlite::db::AsP2pAgentStoreConExt;
            let permit = db.conn_permit().await;
            let task = tokio::task::spawn_blocking(move || {
                let mut conn = db.with_permit(permit)?;
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
            let db = self.spaces.p2p_metrics_db(&DnaHash::from_kitsune(&space))?;
            use holochain_sqlite::db::AsP2pMetricStoreConExt;
            let permit = db.conn_permit().await;
            let task = tokio::task::spawn_blocking(move || {
                let mut conn = db.with_permit(permit)?;
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
        let db = self.spaces.p2p_agents_db(&dna_hash);
        async move {
            Ok(super::p2p_agent_store::get_agent_info_signed(db?.into(), space, agent).await?)
        }
        .boxed()
        .into()
    }

    fn query_region_set(
        &self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc_set: Arc<holochain_p2p::dht_arc::DhtArcSet>,
    ) -> KitsuneHostResult<holochain_p2p::dht::region_set::RegionSetLtcs> {
        let dna_hash = DnaHash::from_kitsune(&space);
        async move {
            let topology = self.get_topology(space.clone()).await?;
            let db = self.spaces.dht_db(&dna_hash)?;
            let region_set =
                query_region_set::query_region_set(db, topology.clone(), &self.strat, dht_arc_set)
                    .await?;
            Ok(region_set)
        }
        .boxed()
        .into()
    }

    fn query_size_limited_regions(
        &self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        size_limit: u32,
        regions: Vec<holochain_p2p::dht::region::Region>,
    ) -> KitsuneHostResult<Vec<holochain_p2p::dht::region::Region>> {
        let dna_hash = DnaHash::from_kitsune(&space);
        async move {
            let topology = self.get_topology(space.clone()).await?;
            let db = self.spaces.dht_db(&dna_hash)?;
            Ok(query_size_limited_regions::query_size_limited_regions(
                db, topology, regions, size_limit,
            )
            .await?)
        }
        .boxed()
        .into()
    }

    fn get_topology(&self, space: Arc<kitsune_p2p::KitsuneSpace>) -> KitsuneHostResult<Topology> {
        let dna_hash = DnaHash::from_kitsune(&space);
        let dna_def = self
            .ribosome_store
            .share_mut(|ds| ds.get_dna_def(&dna_hash))
            .ok_or(DnaError::DnaMissing(dna_hash));
        let cutoff = self.tuning_params.danger_gossip_recent_threshold();
        async move { Ok(dna_def?.topology(cutoff)) }.boxed().into()
    }

    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        use holochain_p2p::DhtOpHashExt;

        async move {
            let op = holochain_p2p::WireDhtOpData::decode(op_data.0.clone())?;

            let op_hash = DhtOpHash::with_data_sync(&op.op_data).into_kitsune();

            Ok(op_hash)
        }
        .boxed()
        .into()
    }
}
