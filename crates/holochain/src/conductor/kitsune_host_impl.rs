//! Implementation of the Kitsune Host API

mod query_region_op_hashes;
mod query_region_set;
mod query_size_limited_regions;
use holochain_conductor_api::conductor::ConductorConfig;
use kitsune_p2p_bin_data::KitsuneSpace;
pub use query_region_op_hashes::query_region_op_hashes;
pub use query_region_set::query_region_set;
pub use query_size_limited_regions::query_size_limited_regions;

use std::sync::Arc;

use super::{ribosome_store::RibosomeStore, space::Spaces};
use futures::FutureExt;
use holo_hash::DnaHash;
use holochain_p2p::{
    dht::{spacetime::Topology, ArqStrat},
    DhtOpHashExt, DnaHashExt, FetchContextExt,
};
use holochain_sqlite::prelude::{AsP2pMetricStoreTxExt, AsP2pStateReadExt, AsP2pStateWriteExt};
use holochain_types::{
    prelude::{DhtOpHash, DnaError},
    share::RwShare,
};
use holochain_zome_types::prelude::Timestamp;
use kitsune_p2p::{
    agent_store::AgentInfoSigned,
    dependencies::kitsune_p2p_fetch::{OpHashSized, RoughSized, TransferMethod},
    event::GetAgentInfoSignedEvt,
    KitsuneHost, KitsuneHostResult,
};
use kitsune_p2p_types::metrics::MetricRecord;
use kitsune_p2p_types::{dependencies::lair_keystore_api, KOpData, KOpHash};
use rusqlite::ToSql;

/// Implementation of the Kitsune Host API.
/// Lets Kitsune make requests of Holochain
pub struct KitsuneHostImpl {
    spaces: Spaces,
    config: Arc<ConductorConfig>,
    ribosome_store: RwShare<RibosomeStore>,
    strat: ArqStrat,
    lair_tag: Option<Arc<str>>,
    lair_client: Option<lair_keystore_api::LairClient>,
}

/// Manual Debug implementation to skip non debuggable fields.
impl std::fmt::Debug for KitsuneHostImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KitsuneHostImpl")
            .field("config", &self.config)
            .field("strat", &self.strat)
            .finish()
    }
}

impl KitsuneHostImpl {
    /// Constructor
    pub fn new(
        spaces: Spaces,
        config: Arc<ConductorConfig>,
        ribosome_store: RwShare<RibosomeStore>,
        strat: ArqStrat,
        lair_tag: Option<Arc<str>>,
        lair_client: Option<lair_keystore_api::LairClient>,
    ) -> Arc<Self> {
        Arc::new(Self {
            spaces,
            config,
            ribosome_store,
            strat,
            lair_tag,
            lair_client,
        })
    }
}

impl KitsuneHost for KitsuneHostImpl {
    fn block(&self, input: kitsune_p2p_block::Block) -> KitsuneHostResult<()> {
        async move {
            let result = self.spaces.block(input.into()).await;
            Ok(result?)
        }
        .boxed()
        .into()
    }

    fn unblock(&self, input: kitsune_p2p_block::Block) -> KitsuneHostResult<()> {
        async move {
            let result = self.spaces.unblock(input.into()).await;
            Ok(result?)
        }
        .boxed()
        .into()
    }

    fn is_blocked(
        &self,
        input: kitsune_p2p_block::BlockTargetId,
        timestamp: Timestamp,
    ) -> KitsuneHostResult<bool> {
        async move {
            let result = self.spaces.is_blocked(input.into(), timestamp).await;
            Ok(result?)
        }
        .boxed()
        .into()
    }

    fn peer_extrapolated_coverage(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc_set: holochain_p2p::dht_arc::DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        async move {
            let db = self.spaces.p2p_agents_db(&DnaHash::from_kitsune(&space))?;
            let coverage = db.p2p_extrapolated_coverage(dht_arc_set).await?;
            Ok(coverage)
        }
        .boxed()
        .into()
    }

    fn record_metrics(
        &self,
        space: std::sync::Arc<kitsune_p2p::KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        async move {
            let db = self.spaces.p2p_metrics_db(&DnaHash::from_kitsune(&space))?;
            Ok(db
                .write_async(move |txn| txn.p2p_log_metrics(records))
                .await?)
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

    fn remove_agent_info_signed(
        &self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<bool> {
        let dna_hash = DnaHash::from_kitsune(&space);
        let db = self.spaces.p2p_agents_db(&dna_hash);
        async move { Ok(db?.p2p_remove_agent(&agent).await?) }
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
                query_region_set(db, topology.clone(), &self.strat, dht_arc_set).await?;
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
            let topology = self.get_topology(space).await?;
            let db = self.spaces.dht_db(&dna_hash)?;
            Ok(query_size_limited_regions::query_size_limited_regions(
                db, topology, regions, size_limit,
            )
            .await?)
        }
        .boxed()
        .into()
    }

    fn query_op_hashes_by_region(
        &self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        region: holochain_p2p::dht::region::RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>> {
        let dna_hash = DnaHash::from_kitsune(&space);
        async move {
            let db = self.spaces.dht_db(&dna_hash)?;
            let topology = self.get_topology(space).await?;
            let bounds = region.to_bounds(&topology);
            Ok(query_region_op_hashes::query_region_op_hashes(db.clone(), bounds).await?)
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
        let cutoff = self
            .config
            .network
            .tuning_params
            .danger_gossip_recent_threshold();
        async move { Ok(dna_def?.topology(cutoff)) }.boxed().into()
    }

    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        async move {
            let op = holochain_p2p::WireDhtOpData::decode(op_data.0.clone())?;

            let op_hash = DhtOpHash::with_data_sync(&op.op_data).into_kitsune();

            Ok(op_hash)
        }
        .boxed()
        .into()
    }

    fn check_op_data(
        &self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        op_hash_list: Vec<KOpHash>,
        context: Option<kitsune_p2p::dependencies::kitsune_p2p_fetch::FetchContext>,
    ) -> KitsuneHostResult<Vec<bool>> {
        async move {
            let db = self.spaces.dht_db(&DnaHash::from_kitsune(&space))?;
            let results = db
                .write_async(move |txn| {
                    let mut out = Vec::new();
                    for op_hash in op_hash_list {
                        let op_hash = DhtOpHash::from_kitsune(&op_hash);
                        match txn.query_row(
                            "SELECT 1 FROM DhtOp WHERE hash = ?",
                            [&op_hash],
                            |_row| Ok(()),
                        ) {
                            Ok(_) => {
                                // might be tempted to remove this given we
                                // are currently reflecting publishes,
                                // but we still need this for the delegate
                                // broadcast case.
                                if let Some(context) = context {
                                    if context.has_request_validation_receipt() {
                                        txn.execute(
                                            "UPDATE DhtOp SET require_receipt = ? WHERE DhtOp.hash = ?",
                                            [&true as &dyn ToSql, &op_hash as &dyn ToSql],
                                        )?;
                                    }
                                }
                                out.push(true)
                            }
                            Err(_) => out.push(false),
                        }
                    }
                    holochain_sqlite::prelude::DatabaseResult::Ok(out)
                })
                .await?;

            Ok(results)
        }
        .boxed()
        .into()
    }

    fn handle_op_hash_received(
        &self,
        _space: &KitsuneSpace,
        op_hash: &RoughSized<KOpHash>,
        transfer_method: TransferMethod,
    ) {
        let hash = DhtOpHash::from_kitsune(&op_hash.data());
        aitia::trace!(&hc_sleuth::Event::ReceivedHash {
            by: self.config.sleuth_id(),
            op: hash,
            method: transfer_method,
        });
    }

    fn handle_op_hash_transmitted(
        &self,
        _space: &KitsuneSpace,
        op_hash: &RoughSized<KOpHash>,
        transfer_method: TransferMethod,
    ) {
        let hash = DhtOpHash::from_kitsune(&op_hash.data());
        aitia::trace!(&hc_sleuth::Event::SentHash {
            by: self.config.sleuth_id(),
            op: hash,
            method: transfer_method,
        });
    }

    fn lair_tag(&self) -> Option<Arc<str>> {
        self.lair_tag.clone()
    }

    fn lair_client(&self) -> Option<lair_keystore_api::LairClient> {
        self.lair_client.clone()
    }
}
