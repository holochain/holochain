use super::SweetZome;
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_state::dht_store::DhtStore;
use std::sync::Arc;
/// A reference to a Cell created by a SweetConductor installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone, Debug)]
pub struct SweetCell {
    pub(super) cell_id: CellId,
    pub(super) cell_dht_store: DhtStore,
    pub(super) conductor_config: Arc<ConductorConfig>,
}

impl SweetCell {
    /// Accessor for CellId
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the DHT store for this cell.
    pub fn dht_store(&self) -> &DhtStore {
        &self.cell_dht_store
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

    /// Accessor for ConductorConfig
    pub fn conductor_config(&self) -> Arc<ConductorConfig> {
        self.conductor_config.clone()
    }
}
