use crate::{
    conductor::{
        api::{error::ConductorApiResult, CellConductorApi},
        cell::error::CellResult,
    },
    core::ribosome::WasmRibosome,
};
use holo_hash::*;
use std::hash::{Hash, Hasher};
use sx_state::env::Environment;
use sx_types::{
    autonomic::AutonomicProcess,
    cell::CellId,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};

pub mod error;

impl Hash for Cell {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.id.hash(state);
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// A Cell is a grouping of the resources necessary to run workflows
/// on behalf of an agent. It does not have a lifetime of its own aside
/// from the lifetimes of the resources which it holds references to.
/// Any work it does is through running a workflow, passing references to
/// the resources needed to complete that workflow.
///
/// The [Conductor] manages a collection of Cells, and will call functions
/// on the Cell when a Conductor API method is called (either a
/// [CellConductorApi] or an [AppInterfaceApi])
pub struct Cell {
    id: CellId,
    conductor_api: CellConductorApi,
    state_env: Environment,
}

impl Cell {
    #[allow(dead_code)]
    fn dna_hash(&self) -> &DnaHash {
        &self.id.dna_hash()
    }

    #[allow(dead_code)]
    fn agent_hash(&self) -> &AgentHash {
        &self.id.agent_hash()
    }

    /// Entry point for incoming messages from the network that need to be handled
    pub async fn handle_network_message(
        &self,
        _msg: Lib3hToClient,
    ) -> CellResult<Option<Lib3hToClientResponse>> {
        unimplemented!()
    }

    /// When the Conductor determines that it's time to execute some [AutonomicProcess],
    /// whether scheduled or through an [AutonomicCue], this function gets called
    pub async fn handle_autonomic_process(&self, process: AutonomicProcess) -> CellResult<()> {
        match process {
            AutonomicProcess::SlowHeal => unimplemented!(),
            AutonomicProcess::HealthCheck => unimplemented!(),
        }
    }

    /// Function called by the Conductor
    pub async fn invoke_zome(
        &self,
        _conductor_api: CellConductorApi,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }

    // TODO: tighten up visibility: only WorkflowRunner needs to access this
    pub(crate) fn get_ribosome(&self) -> WasmRibosome {
        unimplemented!()
    }

    // TODO: tighten up visibility: only WorkflowRunner needs to access this
    pub(crate) fn state_env(&self) -> Environment {
        self.state_env.clone()
    }

    // TODO: tighten up visibility: only WorkflowRunner needs to access this
    pub(crate) fn get_conductor_api(&self) -> CellConductorApi {
        self.conductor_api.clone()
    }
}

////////////////////////////////////////////////////////////////////////////////////
// The following is a sketch from the skunkworx phase, and can probably be removed

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Lib3hClientProtocol) -> Result<(), NetError>;
}

#[allow(dead_code)]
/// TODO - this is a shim until we need a real NetError
enum NetError {}
