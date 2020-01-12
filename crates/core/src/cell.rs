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

pub type DnaAddress = Address;

pub type CellId = (DnaAddress, AgentId);
type NetSender = Sender<Lib3hClientProtocol>;

pub struct CellBuilder {
    pub cell_id: CellId,
    pub network_tx: NetSender,
    pub signal_tx: Sender<Signal>,
    pub zome_tx: Sender<ZomeInvocation>,
}

impl From<CellBuilder> for Cell {
    fn from(builder: CellBuilder) -> Self {
        Self {
            active: true,
            cell_id: builder.cell_id,
            network_tx: builder.network_tx,
            signal_tx: builder.signal_tx,
            zome_tx: builder.zome_tx,
        }
    }
}

#[derive(Clone)]
pub struct Cell {
    /// Whether or not to process queues associated with this Cell
    /// (whether or not to poll futures).
    /// Maybe doesn't belong here?
    active: bool,

    /// Unique identifier for this Cell in this Conductor
    cell_id: CellId,

    /// Send a network message
    network_tx: Sender<Lib3hClientProtocol>,

    /// Send a Signal out through a Conductor Interface
    signal_tx: Sender<Signal>,

    /// Send a ZomeInvocation up to the Conductor
    zome_tx: Sender<ZomeInvocation>,
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
        workflow::network_handler(msg, self.network_tx.clone()).await
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
