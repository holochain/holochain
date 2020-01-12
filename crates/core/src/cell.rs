use crate::{
    agent::SourceChain,
    shims::{get_cascading_cursor, initialize_source_chain, CascadingCursor},
    types::{Signal, ZomeInvocation},
    workflow,
};
use crossbeam_channel::Sender;
use futures::never::Never;
use holochain_core_types::{agent::AgentId, dna::Dna};
use holochain_persistence_api::cas::content::Address;
use lib3h_protocol::{protocol_client::Lib3hClientProtocol, protocol_server::Lib3hServerProtocol};

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);

/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSend = Sender<Lib3hClientProtocol>;

pub struct CellBuilder {
    pub cell_id: CellId,
    pub tx_network: NetSend,
    pub tx_signal: Sender<Signal>,
    pub tx_zome: Sender<ZomeInvocation>,
}

impl From<CellBuilder> for Cell {
    fn from(builder: CellBuilder) -> Self {
        Self {
            active: true,
            cell_id: builder.cell_id,
            tx_network: builder.tx_network,
            tx_signal: builder.tx_signal,
            tx_zome: builder.tx_zome,
        }
    }
}

#[derive(Clone)]
pub struct Cell {
    /// Unique identifier for this Cell in this Conductor
    cell_id: CellId,

    /// Send a network message
    tx_network: Sender<Lib3hClientProtocol>,

    /// Send a Signal out through a Conductor Interface
    tx_signal: Sender<Signal>,

    /// Send a ZomeInvocation up to the Conductor
    tx_zome: Sender<ZomeInvocation>,
}

impl Cell {
    fn id(&self) -> &CellId {
        &self.cell_id
    }

    pub async fn invoke_zome(&self, invocation: ZomeInvocation) -> Result<(), Never> {
        let source_chain = SourceChain::from_cell_id(self.cell_id.clone())?.as_at_head()?;
        workflow::invoke_zome(invocation, source_chain).await
    }

    pub async fn handle_network_msg(&self, msg: Lib3hServerProtocol) -> Result<(), Never> {
        workflow::network_handler(msg, self.tx_network.clone()).await
    }

    /// Get source chain handle for this Cell, or create one if not yet initialized
    fn _get_or_create_source_chain(&self) -> SourceChain {
        SourceChain::from_cell_id(self.cell_id.clone())
            .unwrap_or_else(|_| initialize_source_chain(&self.cell_id))
    }

    fn get_dna(&self, _cursor: CascadingCursor) -> Dna {
        unimplemented!()
    }
}
