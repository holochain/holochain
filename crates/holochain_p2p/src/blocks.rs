use holo_hash::{AgentPubKey, DnaHash};
use holochain_state::block::{query_are_all_blocked, query_is_blocked};
use holochain_timestamp::Timestamp;
use holochain_types::{
    db::{DbKindConductor, DbWrite},
    prelude::{BlockTargetId, CellId},
};
use kitsune2_api::{
    BlockTarget, Blocks, BlocksFactory, BoxFut, Builder, Config, DynBlocks, K2Error, K2Result,
    SpaceId,
};
use std::sync::Arc;

/// Factory for constructing kitsune2_api Blocks backed by the conductor DB.
/// Uses GetDbConductor to query and persist block state (agents, DNAs, cells, spaces),
/// enabling enforcement of block rules across a SpaceId via the BlocksFactory trait.
pub struct HolochainBlocksFactory {
    /// The conductor database connection getter.
    pub getter: crate::GetDbConductor,
}

impl std::fmt::Debug for HolochainBlocksFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainBlocksFactory").finish()
    }
}

impl BlocksFactory for HolochainBlocksFactory {
    fn default_config(&self, _config: &mut Config) -> K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<Builder>,
        space_id: SpaceId,
    ) -> BoxFut<'static, K2Result<DynBlocks>> {
        let dna_hash = DnaHash::from_k2_space(&space_id);
        let getter = self.getter.clone();
        Box::pin(async move {
            let blocks: DynBlocks = Arc::new(HolochainBlocks::new(dna_hash, getter().await));
            Ok(blocks)
        })
    }
}

/// Block implementation in Holochain.
///
/// Holds the target [`DnaHash`] to construct cell IDs for target agents,
/// and a write handle to the conductor database,
/// enabling queries and mutations of block state for this DNA within the networking layer.
#[derive(Debug)]
pub struct HolochainBlocks {
    dna_hash: DnaHash,
    db: DbWrite<DbKindConductor>,
}

impl HolochainBlocks {
    /// Create a new [`HolochainBlocks`].
    pub fn new(dna_hash: DnaHash, db: DbWrite<DbKindConductor>) -> Self {
        Self { dna_hash, db }
    }
}

impl Blocks for HolochainBlocks {
    fn is_blocked(&self, target: BlockTarget) -> BoxFut<'static, K2Result<bool>> {
        let BlockTarget::Agent(agent_id) = target else {
            return Box::pin(async move { Err(K2Error::other("only agents can be blocked")) });
        };
        let db = self.db.clone();
        let cell_id = CellId::new(self.dna_hash.clone(), AgentPubKey::from_k2_agent(&agent_id));
        Box::pin(async move {
            db.read_async(|txn| {
                query_is_blocked(txn, BlockTargetId::Cell(cell_id), Timestamp::now())
            })
            .await
            .map_err(|err| K2Error::other_src("failed to query block for agent", err))
        })
    }

    fn are_all_blocked(&self, targets: Vec<BlockTarget>) -> BoxFut<'static, K2Result<bool>> {
        let mut cell_ids = Vec::new();
        for target in targets {
            let BlockTarget::Agent(agent_id) = target else {
                return Box::pin(async move { Err(K2Error::other("only agents can be blocked")) });
            };
            cell_ids.push(BlockTargetId::Cell(CellId::new(
                self.dna_hash.clone(),
                AgentPubKey::from_k2_agent(&agent_id),
            )));
        }
        let db = self.db.clone();
        Box::pin(async move {
            db.read_async(|txn| query_are_all_blocked(txn, cell_ids, Timestamp::now()))
                .await
                .map_err(|err| {
                    K2Error::other_src("failed to query blocks for vector of block targets", err)
                })
        })
    }

    fn block(&self, _target: BlockTarget) -> BoxFut<'static, K2Result<()>> {
        // Holochain can insert blocks directly into the conductor database. Blocks created by Kitsune2 are not yet supported
        Box::pin(async move { Ok(()) })
    }
}
