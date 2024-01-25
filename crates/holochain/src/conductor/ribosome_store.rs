use holochain_types::{prelude::*, share::RwShare};
use holochain_zome_types::entry_def::EntryDef;
use std::collections::HashMap;
use tracing::*;

use crate::core::ribosome::{real_ribosome::RealRibosome, RibosomeT};

#[derive(Default)]
pub struct RibosomeStore {
    ribosomes: HashMap<DnaHash, RealRibosome>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

impl RibosomeStore {
    pub fn new() -> RwShare<Self> {
        RwShare::new(RibosomeStore {
            ribosomes: HashMap::new(),
            entry_defs: HashMap::new(),
        })
    }

    pub fn add_ribosome(&mut self, ribosome: RealRibosome) {
        self.ribosomes.insert(ribosome.dna_hash().clone(), ribosome);
    }

    pub fn add_ribosomes<T: IntoIterator<Item = (DnaHash, RealRibosome)> + 'static>(
        &mut self,
        ribosomes: T,
    ) {
        self.ribosomes.extend(ribosomes);
    }

    #[instrument(skip(self))]
    pub fn list(&self) -> Vec<DnaHash> {
        self.ribosomes.keys().cloned().collect()
    }

    #[instrument(skip(self))]
    pub fn get_dna_def(&self, hash: &DnaHash) -> Option<DnaDef> {
        self.ribosomes
            .get(hash)
            .map(|d| d.dna_def().clone().into_content())
    }

    // TODO: use Arc, eliminate cloning
    #[instrument(skip(self))]
    pub fn get_dna_file(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.ribosomes.get(hash).map(|r| r.dna_file().clone())
    }

    pub fn get_ribosome(&self, hash: &DnaHash) -> Option<RealRibosome> {
        self.ribosomes.get(hash).cloned()
    }

    pub fn add_entry_def(&mut self, k: EntryDefBufferKey, entry_def: EntryDef) {
        self.entry_defs.insert(k, entry_def);
    }

    pub fn add_entry_defs<T: IntoIterator<Item = (EntryDefBufferKey, EntryDef)> + 'static>(
        &mut self,
        entry_defs: T,
    ) {
        self.entry_defs.extend(entry_defs);
    }

    pub fn get_entry_def(&self, k: &EntryDefBufferKey) -> Option<EntryDef> {
        self.entry_defs.get(k).cloned()
    }
}

impl DnaStore for RibosomeStore {
    fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile> {
        self.get_dna_file(dna_hash)
    }
}
