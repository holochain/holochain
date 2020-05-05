use crate::{
    conductor::{
        api::{error::ConductorApiResult, CellConductorApi},
        cell::error::CellResult,
    },
    core::{
        ribosome::WasmRibosome,
        workflow::{run_workflow, InvokeZomeWorkflow, InvokeZomeWorkspace, ZomeInvocationResult},
    },
};
use holo_hash::*;
use holochain_state::{env::EnvironmentWrite, prelude::*};
use holochain_types::{
    autonomic::AutonomicProcess, cell::CellId, nucleus::ZomeInvocation, shims::*,
};
use std::hash::{Hash, Hasher};

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
    state_env: EnvironmentWrite,
}

impl Cell {
    #[allow(dead_code)]
    fn dna_hash(&self) -> &DnaHash {
        &self.id.dna_hash()
    }

    #[allow(dead_code)]
    fn agent_pubkey(&self) -> &AgentPubKey {
        &self.id.agent_pubkey()
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
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        let arc = self.state_env();
        let env = arc.guard().await;
        let workflow = InvokeZomeWorkflow {
            api: self.conductor_api.clone(),
            ribosome: self.get_ribosome(),
            invocation,
        };
        let workspace = InvokeZomeWorkspace::new(&env.reader()?, &env)?;
        Ok(run_workflow(self.state_env(), workflow, workspace)
            .await
            .map_err(Box::new)?)
    }

    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) fn get_ribosome(&self) -> WasmRibosome {
        unimplemented!()
    }

    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) fn state_env(&self) -> EnvironmentWrite {
        self.state_env.clone()
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
