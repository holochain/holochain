use crate::agent::error::{SourceChainError, SourceChainResult};
use crate::cell::Cell;
use crate::cell::CellApi;
use crate::cursor::ChainCursor;
use crate::cursor::ChainCursorRw;
use crate::cursor::ChainPersistenceManager;
use sx_types::agent::AgentId;
use sx_types::chain_header::ChainHeader;
use sx_types::dna::Dna;
use sx_types::error::SkunkResult;
use sx_types::prelude::*;
use sx_types::shims::*;

pub struct SourceChain<'a, Cursor: ChainCursor> {
    manager: &'a ChainPersistenceManager,
}

impl<'a, Cursor: ChainCursor> SourceChain<'a, Cursor> {
    pub(crate) fn new(manager: &'a ChainPersistenceManager) -> Self {
        Self { manager }
    }

    pub fn now(&self) -> SourceChainSnapshot<Cursor> {
        let reader = self.manager.reader().unwrap();
        let head = unimplemented!(); // reader.query_eav(());
        SourceChainSnapshot {
            reader: self.manager.reader().unwrap(),
            head,
        }
    }

    pub fn as_at(&self, head: Address) -> SourceChainSnapshot<Cursor> {
        SourceChainSnapshot {
            reader: self.manager.reader(),
            head,
        }
    }
    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit<Writer: ChainCursorRw>(&self, writer: Writer) -> SkunkResult<()> {
        unimplemented!()
    }

    pub fn dna(&self) -> SkunkResult<Dna> {
        unimplemented!()
    }

    pub fn agent_id(&self) -> SkunkResult<AgentId> {
        unimplemented!()
    }
}

/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as-at
/// to make sure the right balance is struck between
/// creating as-at snapshots and having access to the actual current source chain
pub struct SourceChainSnapshot<Cursor: ChainCursor> {
    reader: Cursor,
    head: Address,
}

impl<Cursor: ChainCursor> SourceChainSnapshot<Cursor> {
    /// Fails if a source chain has not yet been created for this CellId.
    fn new(reader: Cursor, head: Address) -> SourceChainResult<Self> {
        match reader.contains(&head) {
            Ok(true) => Ok(Self { reader, head }),
            Ok(false) => Err(SourceChainError::MissingHead),
            Err(_) => Err(SourceChainError::ChainNotInitialized),
        }
    }

    pub fn is_initialized(&self) -> bool {
        unimplemented!()
    }
}

pub struct SourceChainBackwardIterator<Cursor: ChainCursor> {
    reader: ChainCursor,
    current: Option<Address>,
}

/// Follows ChainHeader.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl<Cursor: ChainCursor> Iterator for SourceChainBackwardIterator<Cursor> {
    type Item = ChainHeader;

    /// May panic if there is an underlying error in the table
    fn next(&mut self) -> Option<ChainHeader> {
        match &self.current {
            None => None,
            Some(address) => {
                let content = self
                    .reader
                    .get_content(address)
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
