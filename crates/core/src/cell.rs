use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, types::ZomeInvocation, workflow};
use async_trait::async_trait;
use sx_types::agent::AgentId;
use sx_types::error::SkunkResult;
use sx_types::prelude::*;
use sx_types::shims::*;
use crate::txn::source_chain;
use crate::txn::source_chain::Attribute;
use crate::txn::dht::DhtPersistence;
use std::hash::{Hash, Hasher};
use holochain_persistence_api::txn::CursorProvider;
use holochain_persistence_api::error::*;


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


impl Hash for Cell {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        (&self.dna_address, &self.agent_id).hash(state);
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        (&self.dna_address, &self.agent_id) == (&other.dna_address, &other.agent_id)
    }
}

#[derive(Clone)]
pub struct Cell {
    dna_address: DnaAddress, 
    agent_id: AgentId, 
    source_chain_persistence : source_chain::SourceChainPersistence,
    dht_persistence: DhtPersistence
}

#[async_trait]

impl CellApi for Cell {
    fn dna_address(&self) -> &DnaAddress {
        &self.dna_address
    }

    fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult> {
        let source_chain = SourceChain::from_cell(self.clone())?.as_at_head()?;
        let cursor_rw = self.create_cursor_rw()?;
        workflow::invoke_zome(invocation, source_chain, cursor_rw).await
    }

    async fn handle_network_message(
        &self,
        msg: Lib3hToClient,
    ) -> SkunkResult<Option<Lib3hToClientResponse>> {
        workflow::handle_network_message(msg).await
    }
}

impl CursorProvider<Attribute> for Cell {

    fn create_cursor(&self) -> PersistenceResult<source_chain::Cursor> {
        self.source_chain_persistence.create_cursor() 
    }

    fn create_cursor_rw(&self) -> PersistenceResult<source_chain::CursorRw> {
        self.source_chain_persistence.create_cursor_rw() 
    }
}

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Lib3hClientProtocol) -> SkunkResult<()>;
}

/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSender = futures::channel::mpsc::Sender<Lib3hClientProtocol>;
