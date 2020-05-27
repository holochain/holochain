use error::DnaStoreResult;
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Readable, Reader, Writer},
};
use holochain_types::{
    dna::{DnaDef, DnaDefHashed, DnaFile},
    prelude::*,
};
use mockall::automock;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore(HashMap<DnaHash, DnaFile>);

pub type DnaDefCas<'env> = CasBuf<'env, DnaDefHashed>;
pub struct DnaDefBuf<'env> {
    dna_defs: DnaDefCas<'env>,
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

impl<'env> DnaDefBuf<'env> {
    pub fn new(reader: &'env Reader<'env>, dna_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self {
            dna_defs: DnaDefCas::new(reader, dna_def_store)?,
        })
    }

    pub async fn get(&self, dna_hash: &DnaHash) -> DatabaseResult<Option<DnaDefHashed>> {
        self.dna_defs.get(&dna_hash.clone().into()).await
    }

    pub async fn put(&mut self, dna_def: DnaDef) -> DatabaseResult<()> {
        Ok(self.dna_defs.put(DnaDefHashed::with_data(dna_def).await?))
    }

    pub fn iter(&'env self) -> DatabaseResult<impl Iterator<Item = DnaDefHashed> + 'env> {
        // Don't want to pay for deserializing the keys
        Ok(self.dna_defs.iter_raw()?)
    }
}

impl<'env> BufferedStore<'env> for DnaDefBuf<'env> {
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
