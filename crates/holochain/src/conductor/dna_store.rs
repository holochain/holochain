use mockall::automock;
use std::collections::HashMap;
use sx_types::{dna::Dna, prelude::*};
use error::DnaStoreResult;

/// Placeholder for real cache
pub struct RealDnaStore(HashMap<Address, Dna>);

#[automock]
pub trait DnaStore: Send + Sync {
    fn add(&mut self, dna: Dna) -> DnaStoreResult<()>;
    // TODO: FAST: Make this return an iterator to avoid allocating
    fn list(&self) -> Vec<Address>;
}

impl DnaStore for RealDnaStore {
    fn add(&mut self, dna: Dna) -> DnaStoreResult<()> {
        self.0.insert(dna.address(), dna);
        Ok(())
    }
    fn list(&self) -> Vec<Address> {
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
