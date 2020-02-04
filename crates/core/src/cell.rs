use crate::{
    agent::SourceChain,
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    txn::{dht::DhtPersistence, source_chain},
    workflow,
};
use async_trait::async_trait;
use holochain_persistence_api::txn::CursorProvider;
use std::hash::{Hash, Hasher};
use sx_types::{agent::AgentId, error::SkunkResult, prelude::*, shims::*};

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);

/// Might be overkill to have a trait
#[async_trait]
pub trait CellApi: Send + Sync {
    fn dna_address(&self) -> &DnaAddress;
    fn agent_id(&self) -> &AgentId;
    fn cell_id(&self) -> CellId {
        (self.dna_address().clone(), self.agent_id().clone())
    }

    fn source_chain(&self) -> SourceChain;

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult>;
    async fn handle_network_message(
        &self,
        msg: Lib3hToClient,
    ) -> SkunkResult<Option<Lib3hToClientResponse>>;
}

impl Hash for Cell {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.dna_address(), self.agent_id()).hash(state);
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Clone)]
pub struct Cell {
    id: CellId,
    persistence: source_chain::SourceChainPersistence,
    dht_persistence: DhtPersistence,
}

#[async_trait]
impl CellApi for Cell {
    fn dna_address(&self) -> &DnaAddress {
        &self.id.0
    }

    fn agent_id(&self) -> &AgentId {
        &self.id.1
    }

    fn source_chain(&self) -> SourceChain {
        SourceChain::new(&self.persistence)
    }

    async fn invoke_zome(&self, invocation: ZomeInvocation) -> SkunkResult<ZomeInvocationResult> {
        let source_chain = SourceChain::new(&self.persistence);
        let cursor_rw = self.persistence.create_cursor_rw()?;
        workflow::invoke_zome(invocation, source_chain, cursor_rw).await
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

/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSender = futures::channel::mpsc::Sender<Lib3hClientProtocol>;
