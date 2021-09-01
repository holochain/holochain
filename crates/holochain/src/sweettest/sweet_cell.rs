use crate::test_utils::gossip_fixtures::GOSSIP_FIXTURES;

use super::SweetZome;
use hdk::prelude::*;
use holo_hash::*;
use holochain_p2p::HolochainP2pCell;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_types::prelude::*;
use kitsune_p2p::{actor::TestBackdoor, test_util::scenario_def::CoarseLoc};
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

#[cfg(feature = "test_utils")]
use holochain_p2p::{dht_arc::ArcInterval, AgentPubKeyExt};
#[cfg(feature = "test_utils")]
use holochain_sqlite::db::*;

#[cfg(feature = "test_utils")]
impl SweetCell {
    /// Coerce the agent's storage arc to the specified value.
    /// The arc need not be centered on the agent's DHT location, which is
    /// typically a requirement "in the real world", but this can be useful
    /// for integration tests of gossip.
    pub async fn set_storage_arc(&self, arc: ArcInterval) {
        use holochain_p2p::HolochainP2pCellT;

        let agent = self.cell_id.agent_pubkey().to_kitsune();
        self.network
            .test_backdoor(TestBackdoor::SetArc(agent, arc))
            .await
            .unwrap();
    }

    /// Inject ops from the GOSSIP_FIXTURES, indexed by signed (+/-) location.
    /// - This sets the author to the current agent. NB this can lead to
    ///   multiple agents claiming authorship over the same op! For gossip
    ///   purposes, this isn't a problem.
    /// - NB this **removes all other existing ops and related data**!
    pub fn populate_fixture_ops<L>(&self, locations: L)
    where
        L: Iterator<Item = CoarseLoc>,
    {
        let locations: Vec<_> = locations.collect();
        self.cell_env
            .conn()
            .unwrap()
            .with_commit_sync(|txn| {
                // Completely clear all existing cell data, including genesis data
                let (ops_deleted, _, _) = holochain_state::mutations::clear_vault_with_txn(txn);
                // The purpose of this is to clear out genesis ops, so make sure we did that
                assert!(dbg!(ops_deleted) >= 7);

                // Add in fixture data
                for loc in locations.iter() {
                    let op = GOSSIP_FIXTURES.ops.get(*loc).clone();
                    holochain_state::mutations::insert_op(txn, op, true).unwrap();
                }
                // Set author to this agent
                txn.execute(
                    "
                    UPDATE Header
                    SET author = :author, private_entry = 0
                    ",
                    rusqlite::named_params! {
                        ":author": self.agent_pubkey()
                    },
                )?;
                txn.execute(
                    "
                    UPDATE DhtOp
                    SET authored_timestamp_ms = 1111
                    ",
                    [],
                )?;
                DatabaseResult::Ok(())
            })
            .unwrap();
        self.cell_env
            .conn()
            .unwrap()
            .with_reader(|txn| {
                let count: usize =
                    txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))?;
                assert_eq!(count, locations.len());
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
