use super::entry_def_store::EntryDefBufferKey;
use fallible_iterator::FallibleIterator;
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Reader, Writer},
};
use holochain_types::{
    dna::{DnaDef, DnaDefHashed, DnaFile},
    prelude::*,
};
use holochain_zome_types::entry_def::EntryDef;
use mockall::automock;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore {
    dnas: HashMap<DnaHash, DnaFile>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

pub type DnaDefCas<'env> = CasBuf<'env, DnaDef>;
pub struct DnaDefBuf<'env> {
    dna_defs: DnaDefCas<'env>,
}

#[automock]
pub trait DnaStore: Default + Send + Sync {
    fn add(&mut self, dna: DnaFile);
    fn add_dnas<T: IntoIterator<Item = (DnaHash, DnaFile)> + 'static>(&mut self, dnas: T);
    fn add_entry_def(&mut self, k: EntryDefBufferKey, entry_def: EntryDef);
    fn add_entry_defs<T: IntoIterator<Item = (EntryDefBufferKey, EntryDef)> + 'static>(
        &mut self,
        entry_defs: T,
    );
    // TODO: FAST: Make this return an iterator to avoid allocating
    fn list(&self) -> Vec<DnaHash>;
    fn get(&self, hash: &DnaHash) -> Option<DnaFile>;
    fn get_entry_def(&self, k: &EntryDefBufferKey) -> Option<EntryDef>;
}

impl DnaStore for RealDnaStore {
    #[instrument]
    fn add(&mut self, dna: DnaFile) {
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

impl<'env> DnaDefBuf<'env> {
    pub fn new(reader: &'env Reader<'env>, dna_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            dna_defs: DnaDefCas::new(reader, dna_def_store)?,
        })
    }

    pub async fn get(&self, dna_hash: &DnaHash) -> DatabaseResult<Option<DnaDefHashed>> {
        self.dna_defs.get(dna_hash).await
    }

    pub async fn put(&mut self, dna_def: DnaDef) -> DatabaseResult<()> {
        self.dna_defs.put(DnaDefHashed::with_data(dna_def).await?);
        Ok(())
    }

    pub fn get_all(&'env self) -> DatabaseResult<Vec<DnaDefHashed>> {
        self.dna_defs.iter_fail()?.collect()
    }
}

impl<'env> BufferedStore<'env> for DnaDefBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.dna_defs.flush_to_txn(writer)?;
        Ok(())
    }
}
