use fallible_iterator::FallibleIterator;
use holochain_sqlite::buffer::CasBufFreshSync;
use holochain_sqlite::env::EnvironmentRead;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::exports::SingleStore;
use holochain_sqlite::fresh_reader;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::entry_def::EntryDef;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore {
    dnas: HashMap<DnaHash, DnaFile>,
    entry_defs: HashMap<EntryDefBufferKey, EntryDef>,
}

pub struct DnaDefBuf {
    dna_defs: CasBufFreshSync<DnaDef>,
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

impl DnaDefBuf {
    pub fn new(env: EnvironmentRead, dna_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            dna_defs: CasBufFreshSync::new(env, dna_def_store),
        })
    }

    pub async fn get(&self, dna_hash: &DnaHash) -> DatabaseResult<Option<DnaDefHashed>> {
        self.dna_defs.get(dna_hash)
    }

    pub async fn put(&mut self, dna_def: DnaDef) -> DatabaseResult<()> {
        self.dna_defs.put(DnaDefHashed::from_content_sync(dna_def));
        Ok(())
    }

    pub fn get_all(&self) -> DatabaseResult<Vec<DnaDefHashed>> {
        fresh_reader!(self.dna_defs.env(), |r| self
            .dna_defs
            .iter_fail(&r)?
            .collect())
    }
}

impl BufferedStore for DnaDefBuf {
    type Error = DatabaseError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.dna_defs.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
