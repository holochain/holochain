//! Defines a store type for ribosomes and entry definitions.

use holochain_types::{prelude::*, share::RwShare};
use holochain_zome_types::entry_def::EntryDef;
use std::collections::HashMap;

use crate::core::ribosome::{real_ribosome::RealRibosome, RibosomeT};

/// A store for ribosomes and entry definitions.
#[derive(Default)]
pub struct RibosomeStore {
    ribosomes: HashMap<DnaHash, RealRibosome>,
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
    pub fn add_ribosome(&mut self, ribosome: RealRibosome) {
        self.ribosomes.insert(ribosome.dna_hash().clone(), ribosome);
    }

    /// Add ribosomes to the store.
    pub fn add_ribosomes<T: IntoIterator<Item = (DnaHash, RealRibosome)> + 'static>(
        &mut self,
        ribosomes: T,
    ) {
        self.ribosomes.extend(ribosomes);
    }

    /// List all DNA hashes in the store.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn list(&self) -> Vec<DnaHash> {
        self.ribosomes.keys().cloned().collect()
    }

    /// Get the DNA definition for a given DNA hash.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn get_dna_def(&self, hash: &DnaHash) -> Option<DnaDef> {
        self.ribosomes
            .get(hash)
            .map(|d| d.dna_def().clone().into_content())
    }

    /// Get the DNA file for a given DNA hash.
    // TODO: use Arc, eliminate cloning
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub fn get_dna_file(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.ribosomes.get(hash).map(|r| r.dna_file().clone())
    }

    /// Get the ribosome for a given DNA hash.
    pub fn get_ribosome(&self, hash: &DnaHash) -> Option<RealRibosome> {
        self.ribosomes.get(hash).cloned()
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
