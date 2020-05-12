use super::{api::CellConductorApiT, ConductorHandle};
use crate::{
    conductor::{
        api::{error::ConductorApiResult, CellConductorApi},
        cell::error::CellResult,
    },
    core::{
        ribosome::WasmRibosome,
        state::source_chain::SourceChainBuf,
        workflow::{
            run_workflow, GenesisWorkflow, GenesisWorkspace, InvokeZomeWorkflow,
            InvokeZomeWorkspace, ZomeInvocationResult,
        },
    },
};
use error::CellError;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{
    env::{EnvironmentKind, EnvironmentWrite},
    prelude::*,
};
use holochain_types::{
    autonomic::AutonomicProcess, cell::CellId, dna::DnaFile, nucleus::ZomeInvocation, shims::*,
};
use std::{
    hash::{Hash, Hasher},
    path::Path,
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
pub struct Cell<CA = CellConductorApi>
where
    CA: CellConductorApiT,
{
    id: CellId,
    conductor_api: CA,
    state_env: EnvironmentWrite,
}

impl Cell {
    pub async fn create<P: AsRef<Path>>(
        id: CellId,
        conductor_handle: ConductorHandle,
        env_path: P,
        keystore: KeystoreSender,
    ) -> CellResult<Self> {
        let conductor_api = CellConductorApi::new(conductor_handle.clone(), id.clone());
        let state_env = EnvironmentWrite::new(
            env_path.as_ref(),
            EnvironmentKind::Cell(id.clone()),
            keystore,
        )?;
        let source_chain_len = {
            // check if genesis ran on source chain buf
            let env_ref = state_env.guard().await;
            let reader = env_ref.reader()?;
            let source_chain = SourceChainBuf::new(&reader, &env_ref)?;
            source_chain.len()
        };
        let cell = Self {
            id: id.clone(),
            conductor_api,
            state_env,
        };
        // TODO: TK-01747: Make this check more robust
        if source_chain_len == 0 {
            // run genesis
            let dna_file = conductor_handle
                .get_dna(id.dna_hash())
                .await
                .ok_or(CellError::DnaMissing)?;
            cell.genesis(dna_file, id.membrane_proof().clone())
                .await
                .map_err(Box::new)?;
        }
        Ok(cell)
    }

    fn dna_hash(&self) -> &DnaHash {
        &self.id.dna_hash()
    }

    fn agent_pubkey(&self) -> &AgentPubKey {
        &self.id.agent_pubkey()
    }

    pub fn id(&self) -> &CellId {
        &self.id
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
        let reader = env.reader()?;
        let workflow = InvokeZomeWorkflow {
            ribosome: self.get_ribosome().await?,
            invocation,
        };
        let workspace = InvokeZomeWorkspace::new(&reader, &env)?;
        Ok(run_workflow(self.state_env(), workflow, workspace)
            .await
            .map_err(Box::new)?)
    }

    async fn genesis(
        &self,
        dna_file: DnaFile,
        membrane_proof: Option<SerializedBytes>,
    ) -> ConductorApiResult<()> {
        let arc = self.state_env();
        let env = arc.guard().await;
        let reader = env.reader()?;
        let workspace = GenesisWorkspace::new(&reader, &env)?;

        let workflow = GenesisWorkflow::new(
            self.conductor_api.clone(),
            dna_file,
            self.agent_pubkey().clone(),
            membrane_proof,
        );

        Ok(run_workflow(self.state_env(), workflow, workspace)
            .await
            .map_err(Box::new)?)
    }

    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) async fn get_ribosome(&self) -> CellResult<WasmRibosome> {
        match self.conductor_api.get_dna(self.dna_hash()).await {
            Some(dna) => Ok(WasmRibosome::new(dna)),
            None => Err(CellError::DnaMissing),
        }
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
