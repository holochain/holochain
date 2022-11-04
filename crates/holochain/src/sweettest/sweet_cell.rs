use super::SweetZome;
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_sqlite::{
    db::{DbKindAuthored, DbKindDht},
    prelude::DbKindConductor,
};
use holochain_types::db::DbWrite;
/// A reference to a Cell created by a SweetConductor installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone)]
pub struct SweetCell {
    pub(super) cell_id: CellId,
    pub(super) cell_authored_db: DbWrite<DbKindAuthored>,
    pub(super) cell_dht_db: DbWrite<DbKindDht>,
    pub(super) cell_conductor_db: DbWrite<DbKindConductor>,
}

impl SweetCell {
    /// Accessor for CellId
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the authored environment for this cell
    pub fn authored_db(&self) -> &DbWrite<DbKindAuthored> {
        &self.cell_authored_db
    }

    /// Get the dht environment for this cell
    pub fn dht_db(&self) -> &DbWrite<DbKindDht> {
        &self.cell_dht_db
    }

    /// Get the conductor environment for this cell
    pub fn conductor_db(&self) -> &DbWrite<DbKindConductor> {
        &self.cell_conductor_db
    }

    /// Accessor for AgentPubKey
    pub fn agent_pubkey(&self) -> &AgentPubKey {
        self.cell_id.agent_pubkey()
    }

    /// Accessor for DnaHash
    pub fn dna_hash(&self) -> &DnaHash {
        self.cell_id.dna_hash()
    }

    /// Get a SweetZome with the given name
    pub fn zome<Z: Into<ZomeName>>(&self, zome_name: Z) -> SweetZome {
        SweetZome::new(self.cell_id.clone(), zome_name.into())
    }
}

impl std::fmt::Debug for SweetCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SweetCell")
            .field("cell_id", &self.cell_id)
            .finish()
    }
}
