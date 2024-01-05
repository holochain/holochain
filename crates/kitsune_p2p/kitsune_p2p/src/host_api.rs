use kitsune_p2p_fetch::{OpHashSized, RoughSized, TransferMethod};
use kitsune_p2p_timestamp::Timestamp;
use must_future::MustBoxFuture;
use std::sync::Arc;

use kitsune_p2p_types::{
    bin_types::KitsuneSpace,
    dependencies::lair_keystore_api,
    dht::{
        region::{Region, RegionCoords},
        region_set::RegionSetLtcs,
        spacetime::Topology,
    },
    dht_arc::DhtArcSet,
    KOpData, KOpHash,
};

use crate::event::GetAgentInfoSignedEvt;

/// A boxed future result with dynamic error type
pub type KitsuneHostResult<'a, T> =
    MustBoxFuture<'a, Result<T, Box<dyn Send + Sync + std::error::Error>>>;

/// The interface to be implemented by the host, which handles various requests
/// for data
pub trait KitsuneHost: 'static + Send + Sync + std::fmt::Debug {
    /// We are requesting a block.
    fn block(&self, input: kitsune_p2p_block::Block) -> KitsuneHostResult<()>;

    /// We are requesting an unblock.
    fn unblock(&self, input: kitsune_p2p_block::Block) -> KitsuneHostResult<()>;

    /// We want to know if a target is blocked.
    fn is_blocked(
        &self,
        input: kitsune_p2p_block::BlockTargetId,
        timestamp: Timestamp,
    ) -> KitsuneHostResult<bool>;

    /// We need to get previously stored agent info.
    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>>;

    /// Remove an agent info from storage
    fn remove_agent_info_signed(&self, input: GetAgentInfoSignedEvt) -> KitsuneHostResult<bool>;

    /// Extrapolated Peer Coverage.
    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>>;

    /// Query aggregate dht op data to form an LTCS set of region data.
    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs>;

    /// Given an input list of regions, return a list of equal or greater length
    /// such that each region's size is less than the `size_limit`, by recursively
    /// subdividing regions which are over the size limit.
    fn query_size_limited_regions(
        &self,
        space: Arc<KitsuneSpace>,
        size_limit: u32,
        regions: Vec<Region>,
    ) -> KitsuneHostResult<Vec<Region>>;

    /// Get all op hashes within a region
    fn query_op_hashes_by_region(
        &self,
        space: Arc<KitsuneSpace>,
        region: RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>>;

    /// Record a set of metric records.
    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()>;

    /// Get the quantum Topology associated with this Space.
    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Topology>;

    /// Hashing function to get an op_hash from op_data.
    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash>;

    /// Check which hashes we have data for.
    fn check_op_data(
        &self,
        space: Arc<KitsuneSpace>,
        op_hash_list: Vec<KOpHash>,
        _context: Option<kitsune_p2p_fetch::FetchContext>,
    ) -> KitsuneHostResult<Vec<bool>> {
        let _space = space;
        futures::FutureExt::boxed(
            async move { Ok(op_hash_list.into_iter().map(|_| false).collect()) },
        )
        .into()
    }

    /// Do something whenever a batch of op hashes was received and stored in the FetchPool
    // NOTE: currently only needed for aitia, could be removed and the aitia log could be created
    // directly in kitsune.
    fn handle_op_hash_received(
        &self,
        _space: &KitsuneSpace,
        _op_hash: &RoughSized<KOpHash>,
        _transfer_method: TransferMethod,
    ) {
    }

    /// Do something whenever a batch of op hashes was sent to another node
    // NOTE: currently only needed for aitia, could be removed and the aitia log could be created
    // directly in kitsune.
    fn handle_op_hash_transmitted(
        &self,
        _space: &KitsuneSpace,
        _op_hash: &RoughSized<KOpHash>,
        _transfer_method: TransferMethod,
    ) {
    }

    /// Get the lair "tag" identifying the id seed to use for crypto signing.
    /// (this is currently only used in tx5/WebRTC if that feature is enabled.)
    fn lair_tag(&self) -> Option<Arc<str>> {
        None
    }

    /// Get the lair client to use as the backend keystore.
    /// (this is currently only used in tx5/WebRTC if that feature is enabled.)
    fn lair_client(&self) -> Option<lair_keystore_api::LairClient> {
        None
    }
}

/// Trait object for the host interface
pub type HostApi = std::sync::Arc<dyn KitsuneHost>;

/// A HostApi paired with a ghost_actor sender (legacy)
/// When all legacy functions have been moved to the API,
/// this type can be replaced by `HostApi`.
#[derive(Clone, Debug, derive_more::Constructor, derive_more::Deref, derive_more::Into)]
pub struct HostApiLegacy {
    /// The new API
    #[deref]
    pub api: HostApi,
    /// The old ghost_actor sender based API
    pub legacy: futures::channel::mpsc::Sender<crate::event::KitsuneP2pEvent>,
}

// Test-only stub which mostly panics
#[cfg(any(test, feature = "test_utils"))]
mod host_stub;
#[cfg(any(test, feature = "test_utils"))]
pub use host_stub::*;

#[cfg(any(test, feature = "test_utils"))]
mod host_default_error;
#[cfg(any(test, feature = "test_utils"))]
pub use host_default_error::*;
use kitsune_p2p_types::metrics::MetricRecord;
