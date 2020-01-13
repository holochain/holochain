use crate::types::ZomeInvocationResult;
use crate::{
    agent::SourceChain,
    shims::{get_cascading_cursor, initialize_source_chain, CascadingCursor},
    types::{Signal, ZomeInvocation},
    workflow,
};
use async_trait::async_trait;
use crossbeam_channel::Sender;
use futures::never::Never;
use holochain_core_types::{dna::Dna};
use skunkworx_core_types::{agent::AgentId, error::SkunkResult};
use holochain_persistence_api::cas::content::Address;
use lib3h_protocol::{protocol_client::Lib3hClientProtocol, protocol_server::Lib3hServerProtocol};

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);

/// Might be overkill to have a trait
#[async_trait]
pub trait CellApi: Send + PartialEq + std::hash::Hash + Eq {
    fn dna_address(&self) -> &DnaAddress;
    fn agent_id(&self) -> &AgentId;
    fn cell_id(&self) -> CellId {
        (self.dna_address().clone(), self.agent_id().clone())
    }

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult>;
    async fn handle_network_message(&self, msg: Lib3hServerProtocol, tx_network: NetSender) -> SkunkResult<()>;
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
        workflow::invoke_zome(invocation, source_chain).await
    }

    async fn handle_network_message(&self, msg: Lib3hServerProtocol, tx_network: NetSender) -> SkunkResult<()> {
        workflow::network_handler(msg, tx_network).await
    }
}

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Lib3hClientProtocol) -> SkunkResult<()>;
}

trait ChainRead {
    fn chain_read_cursor(&self) -> CascadingCursor;
}

trait ChainWrite {
    fn chain_write_cursor(&self) -> CascadingCursor;
}


/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSender = Sender<Lib3hClientProtocol>;
