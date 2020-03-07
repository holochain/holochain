use super::{ChainInvalidReason, SourceChainBuf, SourceChainError, SourceChainResult};

use shrinkwraprs::Shrinkwrap;
use sx_state::prelude::Reader;
use sx_types::{agent::AgentId, prelude::Address};

type InnerBuffer<'env> = SourceChainBuf<'env, Reader<'env>>;

/// A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
/// It has the same interface as its underlying SourceChainBuf, except that certain operations,
/// which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentId,
/// cannot fail, so the function return types reflect that.
#[derive(Shrinkwrap)]
pub struct SourceChain<'env>(InnerBuffer<'env>);

impl<'env> SourceChain<'env> {
    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        self.0.agent_id()?.ok_or(SourceChainError::InvalidStructure(
            ChainInvalidReason::GenesisDataMissing,
        ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&Address> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }
}

impl<'env> From<InnerBuffer<'env>> for SourceChain<'env> {
    fn from(buffer: InnerBuffer<'env>) -> Self {
        Self(buffer)
    }
}
