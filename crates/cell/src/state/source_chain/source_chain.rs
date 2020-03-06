use super::{ChainInvalidReason, SourceChainBuffer, SourceChainError, SourceChainResult};
use crate::state::{
    chain_cas::{ChainCasBuffer, HeaderCas},
    chain_sequence::ChainSequenceBuffer,
};
use core::ops::Deref;
use sx_state::{
    buffer::StoreBuffer,
    db::{self, DbManager},
    env::ReadManager,
    error::WorkspaceResult,
    prelude::{Readable, Reader, Writer},
};
use sx_types::{
    agent::AgentId,
    chain_header::ChainHeader,
    entry::{entry_type::EntryType, Entry},
    prelude::{Address, AddressableContent},
    signature::{Provenance, Signature},
};

type InnerBuffer<'env> = SourceChainBuffer<'env, Reader<'env>>;

/// A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
/// It has the same interface as its underlying SourceChainBuffer, except that certain operations,
/// which would return Option in the SourceChainBuffer, like getting the source chain head, or the AgentId,
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
