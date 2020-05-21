use error::DnaStoreResult;
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Readable, Reader, Writer},
};
use holochain_types::{
    dna::{DnaDef, DnaFile},
    prelude::*,
};
use mockall::automock;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore(HashMap<DnaHash, DnaFile>);

pub type DnaDefCas<'env, R> = CasBuf<'env, DnaDef, R>;
pub struct DnaDefBuf<'env, R: Readable = Reader<'env>> {
    dna_defs: DnaDefCas<'env, R>,
}

#[automock]
pub trait DnaStore: Default + Send + Sync {
    fn add(&mut self, dna: DnaFile) -> DnaStoreResult<()>;
    fn add_dnas<T: IntoIterator<Item = (DnaHash, DnaFile)> + 'static>(&mut self, dnas: T);
    // TODO: FAST: Make this return an iterator to avoid allocating
    fn list(&self) -> Vec<DnaHash>;
    fn get(&self, hash: &DnaHash) -> Option<DnaFile>;
}

impl DnaStore for RealDnaStore {
    #[instrument]
    fn add(&mut self, dna: DnaFile) -> DnaStoreResult<()> {
        self.0.insert(dna.dna_hash().clone(), dna);
        Ok(())
    }
    fn add_dnas<T: IntoIterator<Item = (DnaHash, DnaFile)> + 'static>(&mut self, dnas: T) {
        self.0.extend(dnas);
    }
    #[instrument]
    fn list(&self) -> Vec<DnaHash> {
        self.0.keys().cloned().collect()
    }
    #[instrument]
    fn get(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.0.get(hash).cloned()
    }
}

impl RealDnaStore {
    pub fn new() -> Self {
        RealDnaStore(HashMap::new())
    }
}

impl<'env, R: Readable> DnaDefBuf<'env, R> {
    pub fn new(reader: &'env R, dna_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            dna_defs: DnaDefCas::new(reader, dna_def_store)?,
        })
    }

    pub fn get(&self, dna_hash: &DnaHash) -> DatabaseResult<Option<DnaDef>> {
        self.dna_defs.get(&dna_hash.clone().into())
    }

    pub async fn put(&mut self, dna_def: DnaDef) -> DatabaseResult<DnaHash> {
        let dna_hash = dna_def.dna_hash().await.clone();
        self.dna_defs.put(dna_hash.clone().into(), dna_def);
        Ok(dna_hash)
    }

    pub fn iter(&'env self) -> DatabaseResult<impl Iterator<Item = DnaDef> + 'env> {
        Ok(self.dna_defs.iter_raw()?.map(|(_, dna_def)| dna_def))
    }
}

impl<'env, R> BufferedStore<'env> for DnaDefBuf<'env, R>
where
    R: Readable,
{
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.dna_defs.flush_to_txn(writer)?;
        Ok(())
    }
}

pub mod error {
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum DnaStoreError {
        #[error("Store failed to write")]
        WriteFail,
    }
    pub type DnaStoreResult<T> = Result<T, DnaStoreError>;
}
