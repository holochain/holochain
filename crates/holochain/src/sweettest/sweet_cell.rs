use super::SweetZome;
use hdk::prelude::*;
use holo_hash::*;
use holochain_p2p::HolochainP2pCell;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_types::prelude::*;
use kitsune_p2p::actor::TestBackdoor;
/// A reference to a Cell created by a SweetConductor installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone, derive_more::Constructor)]
pub struct SweetCell {
    pub(super) cell_id: CellId,
    pub(super) cell_env: EnvWrite,
    pub(super) p2p_agents_env: EnvWrite,
    pub(super) network: HolochainP2pCell,
}

impl SweetCell {
    /// Accessor for CellId
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the environment for this cell
    pub fn env(&self) -> &EnvWrite {
        &self.cell_env
    }

    /// Accessor for AgentPubKey
    pub fn agent_pubkey(&self) -> &AgentPubKey {
        &self.cell_id.agent_pubkey()
    }

    /// Accessor for DnaHash
    pub fn dna_hash(&self) -> &DnaHash {
        &self.cell_id.dna_hash()
    }

    /// Get a SweetZome with the given name
    pub fn zome<Z: Into<ZomeName>>(&self, zome_name: Z) -> SweetZome {
        SweetZome::new(self.cell_id.clone(), zome_name.into())
    }
}

#[cfg(feature = "no-hash-integrity")]
use holochain_p2p::{
    dht_arc::{ArcInterval, DhtLocation},
    AgentPubKeyExt,
};
#[cfg(feature = "no-hash-integrity")]
use holochain_sqlite::db::*;

#[cfg(feature = "no-hash-integrity")]
impl SweetCell {
    /// Coerce the agent's storage arc to the specified value.
    /// The arc need not be centered on the agent's DHT location, which is
    /// typically a requirement "in the real world", but this can be useful
    /// for integration tests of gossip.
    #[cfg(feature = "test_utils")]
    pub async fn set_storage_arc(&self, arc: ArcInterval) {
        use holochain_p2p::HolochainP2pCellT;

        let agent = self.cell_id.agent_pubkey().to_kitsune();
        self.network
            .test_backdoor(TestBackdoor::SetArc(agent, arc))
            .await
            .unwrap();
    }

    /// Inject fake ops into the cell's vault, such that each op is at the
    /// specified location. The locations will not match the op hashes.
    pub fn inject_fake_ops<L>(&self, locations: L)
    where
        L: Iterator<Item = DhtLocation>,
    {
        use ::fixt::prelude::*;

        self.cell_env
            .conn()
            .unwrap()
            .with_commit_sync(|txn| {
                for loc in locations {
                    let signed_header = fixt!(SignedHeaderHashed);
                    let op = DhtOp::StoreElement(
                        signed_header.signature().clone(),
                        signed_header.header().clone(),
                        None,
                    );
                    let mut op = DhtOpHashed::from_content_sync(op);
                    // Override the op hash to be at the
                    // specified location
                    op.override_hash(|hash| hash.set_loc(loc));
                    holochain_state::mutations::insert_op(txn, op, true).unwrap();
                }
                DatabaseResult::Ok(())
            })
            .unwrap();
    }
}

impl std::fmt::Debug for SweetCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SweetCell")
            .field("cell_id", &self.cell_id())
            .finish()
    }
}
