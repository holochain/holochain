use holochain_types::{prelude::*, share::RwShare};
use holochain_zome_types::entry_def::EntryDef;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct DnaStore {
    dnas: HashMap<DnaHash, DnaFile>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

impl DnaStore {
    pub fn new() -> RwShare<Self> {
        RwShare::new(DnaStore {
            dnas: HashMap::new(),
            entry_defs: HashMap::new(),
        })
    }

    #[instrument]
    pub fn add_dna(&mut self, dna: DnaFile) {
        self.dnas.insert(dna.dna_hash().clone(), dna);
    }

    pub fn add_dnas<T: IntoIterator<Item = (DnaHash, DnaFile)> + 'static>(&mut self, dnas: T) {
        self.dnas.extend(dnas);
    }

    #[instrument]
    pub fn list(&self) -> Vec<DnaHash> {
        self.dnas.keys().cloned().collect()
    }

    #[instrument]
    pub fn get_dna_def(&self, hash: &DnaHash) -> Option<DnaDef> {
        self.dnas.get(hash).map(|d| d.dna_def()).cloned()
    }

    #[instrument]
    pub fn get_dna_file(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.dnas.get(hash).cloned()
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
