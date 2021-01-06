use super::CoolZome;
use hdk3::prelude::*;
use holo_hash::DnaHash;
use holochain_lmdb::env::EnvironmentWrite;

/// A reference to a Cell created by a CoolConductor installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone, derive_more::Constructor)]
pub struct CoolCell {
    pub(super) cell_id: CellId,
    pub(super) cell_env: EnvironmentWrite,
}

impl CoolCell {
    /// Accessor for CellId
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the environment for this cell
    pub async fn env(&self) -> &EnvironmentWrite {
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

    /// Get a CoolZome with the given name
    pub fn zome<Z: Into<ZomeName>>(&self, zome_name: Z) -> CoolZome {
        CoolZome::new(self.cell_id.clone(), zome_name.into())
    }
}
