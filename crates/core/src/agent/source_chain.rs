use sx_types::chain_header::ChainHeader;
use crate::{
    agent::error::{SourceChainError, SourceChainResult},
    cell::Cell,
    txn::source_chain,
};
use holochain_persistence_api::{
    cas::content::Address,
    txn::{CursorProvider, Writer},
};
use sx_types::{agent::AgentId, dna::Dna, error::SkunkResult, prelude::*, shims::*};

pub struct SourceChain<'a> {
    persistence: &'a source_chain::SourceChainPersistence,
}

impl<'a> SourceChain<'a> {
    pub(crate) fn new(persistence: &'a source_chain::SourceChainPersistence) -> Self {
        Self { persistence }
    }

    pub fn now(&self) -> SkunkResult<SourceChainSnapshot> {
        let reader = self.persistence.create_cursor()?;
        let head = unimplemented!(); // reader.query_eav(());
        Ok(SourceChainSnapshot { reader, head })
    }

    pub fn as_at(&self, head: Address) -> SkunkResult<SourceChainSnapshot> {
        Ok(SourceChainSnapshot {
            reader: self.persistence.create_cursor()?,
            head,
        })
    }

    pub fn dna(&self) -> SkunkResult<Dna> {
        unimplemented!()
    }

    pub fn agent_id(&self) -> SkunkResult<AgentId> {
        unimplemented!()
    }

    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit(&self, cursor_rw: source_chain::CursorRw) -> SkunkResult<()> {
        Ok(cursor_rw.commit()?)
    }
}

/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as-at
/// to make sure the right balance is struck between
/// creating as-at snapshots and having access to the actual current source chain
pub struct SourceChainSnapshot {
    reader: source_chain::Cursor,
    head: Address,
}

impl SourceChainSnapshot {
    /// Fails if a source chain has not yet been created for this CellId.
    fn new(reader: source_chain::Cursor, head: Address) -> SourceChainResult<Self> {
        match reader.contains(&head) {
            Ok(true) => Ok(Self { reader, head }),
            Ok(false) => Err(SourceChainError::MissingHead),
            Err(_) => Err(SourceChainError::ChainNotInitialized),
        }
    }

    pub fn is_initialized(&self) -> bool {
        unimplemented!()
        // for header in self.iter_back() {

        // }
    }

    pub fn iter_back(&self) -> SourceChainBackwardIterator {
        SourceChainBackwardIterator {
            reader: self.reader.clone(),
            current: Some(self.head.clone()),
        }
    }
}

pub struct SourceChainBackwardIterator {
    reader: source_chain::Cursor,
    current: Option<Address>,
}

/// Follows ChainHeader.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl Iterator for SourceChainBackwardIterator {
    type Item = ChainHeader;

    /// May panic if there is an underlying bad entry in the CAS
    /// This is a pretty major problem, but we shouldn't crash the entire conductor for it.
    /// TODO: could use fallible iterator
    fn next(&mut self) -> Option<ChainHeader> {
        match &self.current {
            None => None,
            Some(address) => {
                let content = self
                    .reader
                    .fetch(address)
                    .expect("Could not access source chain store!")
                    .expect(&format!(
                        "No content found in source chain store at address {}",
                        address
                    ));
                let header = ChainHeader::try_from_content(&content).expect(&format!(
                    "Invalid content in source chain store at address {}",
                    address
                ));
                self.current = header.link();
                Some(header)
            }
        }
    }
}
