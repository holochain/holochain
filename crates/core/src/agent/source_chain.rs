use crate::cursor::ChainCursorX;
use crate::cursor::SourceChainAttribute;
use crate::agent::error::{SourceChainError, SourceChainResult};
use crate::cursor::ChainCursorManagerX;
use crate::cell::Cell;
use crate::cell::CellApi;
use crate::cursor::CursorR;
use crate::cursor::CursorRw;
use holochain_persistence_api::cas::content::Address;
use sx_types::agent::AgentId;
use sx_types::error::SkunkResult;
use sx_types::shims::*;

pub struct SourceChain<'a> {
    manager: &'a ChainCursorManagerX,
}

impl<'a> SourceChain<'a> {
    pub(crate) fn new(manager: &'a ChainCursorManagerX) -> Self {
        Self { manager }
    }

    pub fn now(&self) -> SourceChainSnapshot {
        let reader = self.manager.reader();
        let head = unimplemented!(); // reader.query_eav(());
        SourceChainSnapshot {
            reader: self.manager.reader(),
            head
        }
    }

    pub fn as_at(&self, head: Address) -> SourceChainSnapshot {
        SourceChainSnapshot {
            reader: self.manager.reader(),
            head
        }
    }
    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit<Writer: CursorRw<SourceChainAttribute>>(&self, writer: Writer) -> SkunkResult<()> {
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
pub struct SourceChainSnapshot {
    reader: ChainCursorX,
    head: Address,
}

impl SourceChainSnapshot {
    /// Fails if a source chain has not yet been created for this CellId.
    fn new(reader: ChainCursorX, head: Address) -> SourceChainResult<Self> {
        match reader.contains_content(&head) {
            Ok(true) => Ok(Self { reader, head }),
            Ok(false) => Err(SourceChainError::MissingHead),
            Err(_) => Err(SourceChainError::ChainNotInitialized)
        }
    }

    pub fn is_initialized(&self) -> bool {
        unimplemented!()
    }
}
