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
    reader: source_chain::CursorRw,
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
    }
}
