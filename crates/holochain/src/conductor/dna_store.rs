use error::DnaStoreResult;
use holochain_types::{dna::Dna, prelude::*};
use mockall::automock;
use std::collections::HashMap;

/// Placeholder for real dna store
#[derive(Default)]
pub struct RealDnaStore(HashMap<DnaHash, Dna>);

#[automock]
pub trait DnaStore: Default + Send + Sync {
    fn add(&mut self, dna: Dna) -> DnaStoreResult<()>;
    // TODO: FAST: Make this return an iterator to avoid allocating
    fn list(&self) -> Vec<DnaHash>;
}

impl DnaStore for RealDnaStore {
    fn add(&mut self, dna: Dna) -> DnaStoreResult<()> {
        self.0.insert(dna.dna_hash(), dna);
        Ok(())
    }
    fn list(&self) -> Vec<DnaHash> {
        self.0.keys().cloned().collect()
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
