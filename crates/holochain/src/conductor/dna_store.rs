use holochain_types::prelude::*;
use holochain_zome_types::entry_def::EntryDef;
use rusqlite::named_params;
use rusqlite::OptionalExtension;
use rusqlite::Transaction;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore {
    dnas: HashMap<DnaHash, DnaFile>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

impl DnaStore for RealDnaStore {
    #[instrument]
    fn add_dna(&mut self, dna: DnaFile) {
        self.dnas.insert(dna.dna_hash().clone(), dna);
    }
    fn add_dnas<T: IntoIterator<Item = (DnaHash, DnaFile)> + 'static>(&mut self, dnas: T) {
        self.dnas.extend(dnas);
    }
    #[instrument]
    fn list(&self) -> Vec<DnaHash> {
        self.dnas.keys().cloned().collect()
    }
    #[instrument]
    fn get(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.dnas.get(hash).cloned()
    }
    fn add_entry_def(&mut self, k: EntryDefBufferKey, entry_def: EntryDef) {
        self.entry_defs.insert(k, entry_def);
    }
    fn add_entry_defs<T: IntoIterator<Item = (EntryDefBufferKey, EntryDef)> + 'static>(
        &mut self,
        entry_defs: T,
    ) {
        self.entry_defs.extend(entry_defs);
    }
    fn get_entry_def(&self, k: &EntryDefBufferKey) -> Option<EntryDef> {
        self.entry_defs.get(k).cloned()
    }
}

impl RealDnaStore {
    pub fn new() -> Self {
        RealDnaStore {
            dnas: HashMap::new(),
            entry_defs: HashMap::new(),
        }
    }
}
