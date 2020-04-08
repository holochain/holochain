//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentId,
//! cannot fail, so the function return types reflect that.

use shrinkwraprs::Shrinkwrap;
use sx_state::{db::DbManager, error::DatabaseResult, prelude::Readable};
use sx_types::{agent::AgentId, prelude::Address};

pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
pub struct SourceChain<'env, R: Readable>(SourceChainBuf<'env, R>);

impl<'env, R: Readable> SourceChain<'env, R> {
    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        self.0.agent_id()?.ok_or(SourceChainError::InvalidStructure(
            ChainInvalidReason::GenesisDataMissing,
        ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&Address> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }
    pub fn new(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(reader, dbs)?.into())
    }
}

impl<'env, R: Readable> From<SourceChainBuf<'env, R>> for SourceChain<'env, R> {
    fn from(buffer: SourceChainBuf<'env, R>) -> Self {
        Self(buffer)
    }
}
