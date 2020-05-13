use error::DnaStoreResult;
use holochain_types::{dna::DnaFile, prelude::*};
use mockall::automock;
use std::collections::HashMap;
use tracing::*;

/// Placeholder for real dna store
#[derive(Default, Debug)]
pub struct RealDnaStore(HashMap<DnaHash, DnaFile>);

#[automock]
pub trait DnaStore: Default + Send + Sync {
    fn add(&mut self, dna: DnaFile) -> DnaStoreResult<()>;
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

pub mod error {
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum DnaStoreError {
        #[error("Store failed to write")]
        WriteFail,
    }
    pub type DnaStoreResult<T> = Result<T, DnaStoreError>;
}
