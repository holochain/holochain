use fallible_iterator::FallibleIterator;
use holochain_lmdb::buffer::CasBufFreshAsync;
use holochain_lmdb::env::EnvironmentRead;
use holochain_lmdb::error::DatabaseError;
use holochain_lmdb::error::DatabaseResult;
use holochain_lmdb::exports::SingleStore;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_types::prelude::*;
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

pub struct DnaDefBuf {
    dna_defs: CasBufFreshAsync<DnaDef>,
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
            dna_defs: CasBufFreshAsync::new(env, dna_def_store),
        })
    }

    pub async fn get(&self, dna_hash: &DnaHash) -> DatabaseResult<Option<DnaDefHashed>> {
        self.dna_defs.get(dna_hash).await
    }

    pub async fn put(&mut self, dna_def: DnaDef) -> DatabaseResult<()> {
        self.dna_defs.put(DnaDefHashed::from_content(dna_def).await);
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
