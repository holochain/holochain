//! Defines a store type for ribosomes and entry definitions.

use holochain_types::{prelude::*, share::RwShare};
use holochain_zome_types::entry_def::EntryDef;
use std::collections::HashMap;

use crate::core::ribosome::{real_ribosome::RealRibosome, RibosomeT};

/// A store for ribosomes and entry definitions.
#[derive(Default)]
pub struct RibosomeStore {
    ribosomes: HashMap<CellId, RealRibosome>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

impl RibosomeStore {
    /// Create a new ribosome store.
    pub fn new() -> RwShare<Self> {
        RwShare::new(RibosomeStore {
            ribosomes: HashMap::new(),
            entry_defs: HashMap::new(),
        })
    }

    /// Add a single ribosome to the store.
    pub fn add_ribosome(&mut self, cell_id: CellId, ribosome: RealRibosome) {
        self.ribosomes.insert(cell_id, ribosome);
    }

    /// Add ribosomes to the store.
    pub fn add_ribosomes<T: IntoIterator<Item = (CellId, RealRibosome)> + 'static>(
        &mut self,
        ribosomes: T,
    ) {
        self.ribosomes.extend(ribosomes);
    }

    /// List all DNA hashes in the store.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn list_dna_hashes(&self) -> Vec<DnaHash> {
        self.ribosomes
            .keys()
            .cloned()
            .map(|c| c.dna_hash().clone())
            .collect()
    }

    /// Get the DNA definition for a given CellId.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn get_dna_def(&self, cell_id: &CellId) -> Option<DnaDef> {
        self.ribosomes
            .get(cell_id)
            .map(|d| d.dna_def_hashed().clone().into_content())
    }

    /// Get the DNA file for a given CellId.
    // TODO: use Arc, eliminate cloning
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn get_dna_file(&self, cell_id: &CellId) -> Option<DnaFile> {
        self.ribosomes.get(cell_id).map(|r| r.dna_file().clone())
    }

    /// Get the ribosome for a given CellId.
    pub fn get_ribosome(&self, cell_id: &CellId) -> Option<RealRibosome> {
        self.ribosomes.get(cell_id).cloned()
    }

    /// Get any ribosome associated to a CellId matching the given DnaHash.
    pub fn get_any_ribosome_for_dna_hash(&self, dna_hash: &DnaHash) -> Option<RealRibosome> {
        if let Some(cell_id) = self.ribosomes.keys().find(|c| c.dna_hash() == dna_hash) {
            return self.ribosomes.get(cell_id).cloned();
        }
        None
    }

    /// Add a single [`EntryDef`] to the store.
    pub fn add_entry_def(&mut self, k: EntryDefBufferKey, entry_def: EntryDef) {
        self.entry_defs.insert(k, entry_def);
    }

    /// Add new [`EntryDef`]s to the store.
    pub fn add_entry_defs<T: IntoIterator<Item = (EntryDefBufferKey, EntryDef)> + 'static>(
        &mut self,
        entry_defs: T,
    ) {
        self.entry_defs.extend(entry_defs);
    }

    /// Get an [`EntryDef`] by its key.
    pub fn get_entry_def(&self, k: &EntryDefBufferKey) -> Option<EntryDef> {
        self.entry_defs.get(k).cloned()
    }
}
