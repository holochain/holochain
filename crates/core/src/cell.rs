use crate::cursor::CasCursorX;
use crate::cursor::CursorR;
use crate::cursor::CursorRw;
use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, types::ZomeInvocation, workflow};
use async_trait::async_trait;
use sx_types::agent::AgentId;
use sx_types::error::SkunkResult;
use sx_types::prelude::*;
use sx_types::shims::*;

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);

/// Might be overkill to have a trait
#[async_trait]
pub trait CellApi: Send + Sync + PartialEq + std::hash::Hash + Eq {
    fn dna_address(&self) -> &DnaAddress;
    fn agent_id(&self) -> &AgentId;
    fn cell_id(&self) -> CellId {
        (self.dna_address().clone(), self.agent_id().clone())
    }

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult>;
    async fn handle_network_message(
        &self,
        msg: Lib3hToClient,
    ) -> SkunkResult<Option<Lib3hToClientResponse>>;
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Cell(DnaAddress, AgentId);

#[async_trait]

impl CellApi for Cell {
    fn dna_address(&self) -> &DnaAddress {
        &self.0
    }

    fn agent_id(&self) -> &AgentId {
        &self.1
    }

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult> {
        let source_chain = SourceChain::from_cell(self.clone())?.as_at_head()?;
        let cursor = CasCursorX;
        workflow::invoke_zome(invocation, source_chain, cursor).await
    }

    async fn handle_network_message(
        &self,
        msg: Lib3hToClient,
    ) -> SkunkResult<Option<Lib3hToClientResponse>> {
        workflow::handle_network_message(msg).await
    }
}

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Lib3hClientProtocol) -> SkunkResult<()>;
}

trait ChainRead {
    fn chain_read_cursor<C: CursorR>(&self) -> C;
}

trait ChainWrite {
    fn chain_write_cursor<C: CursorRw>(&self) -> C;
}

/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSender = futures::channel::mpsc::Sender<Lib3hClientProtocol>;
