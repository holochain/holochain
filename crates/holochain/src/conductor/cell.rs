use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::handle::ConductorHandle;
use crate::core::ribosome::ZomeCallInvocation;
use crate::{
    conductor::{
        api::{error::ConductorApiResult, CellConductorApi},
        cell::error::CellResult,
    },
    core::ribosome::{guest_callback::init::InitResult, wasm_ribosome::WasmRibosome},
    core::{
        state::source_chain::SourceChainBuf,
        workflow::{
            run_workflow, GenesisWorkflow, GenesisWorkspace, InitializeZomesWorkflow,
            InitializeZomesWorkspace, InvokeZomeWorkflow, InvokeZomeWorkspace,
            ZomeCallInvocationResult,
        },
    },
};
use error::CellError;
use holo_hash::*;
use holochain_keystore::KeystoreSender;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::env::{EnvironmentKind, EnvironmentWrite, ReadManager};
use holochain_types::{autonomic::AutonomicProcess, cell::CellId, prelude::Todo, Header};
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

        // get the environment
        let state_env = EnvironmentWrite::new(
            env_path.as_ref(),
            EnvironmentKind::Cell(id.clone()),
            keystore,
        )?;

        // check if genesis has been run
        let source_chain_len = {
            // check if genesis ran on source chain buf
            let env_ref = state_env.guard().await;
            let reader = env_ref.reader()?;
            let source_chain = SourceChainBuf::new(&reader, &env_ref)?;

            // Check if initialization has run
            if let Some(Header::InitZomesComplete(_)) = source_chain.get_index(4).await? {
            } else {
                // If not run it
                // TODO: TK-01852 Run this when initializa zomes is complete
                let run_init = tokio::spawn({
                    let state_env = state_env.clone();
                    let id = id.clone();
                    async move {
                        let env_ref = state_env.guard().await;
                        let reader = env_ref.reader()?;
                        // Create the workspace
                        let workspace = InvokeZomeWorkspace::new(&reader, &env_ref)?;
                        let workspace = InitializeZomesWorkspace(workspace);

                        // get the dna
                        let dna_file = conductor_handle
                            .get_dna(id.dna_hash())
                            .await
                            .ok_or(CellError::DnaMissing)?;

                        // Create the workflow and run it
                        let workflow = InitializeZomesWorkflow {
                            agent_key: id.agent_pubkey().clone(),
                            dna_file,
                        };
                        run_workflow(state_env.clone(), workflow, workspace).await
                    }
                });
                let init_result = run_init.await?.map_err(Box::new)??;
                match init_result {
                    InitResult::Pass => (),
                    r @ _ => return Err(CellError::InitFailed(r)),
                }
            }

            source_chain.len()
        };

        // TODO: TK-01747: Make this check more robust
        if source_chain_len == 4 {
            Err(CellError::CellWithoutGenesis(id))
        } else {
            Ok(Self {
                id,
                conductor_api,
                state_env,
            })
        }
    }

    /// Must be run before creating a cell
    pub async fn genesis<P: AsRef<Path>>(
        id: CellId,
        conductor_handle: ConductorHandle,
        env_path: P,
        keystore: KeystoreSender,
        membrane_proof: Option<SerializedBytes>,
    ) -> CellResult<EnvironmentWrite> {
        // create the environment
        let state_env = EnvironmentWrite::new(
            env_path.as_ref(),
            EnvironmentKind::Cell(id.clone()),
            keystore,
        )?;

        // get a reader
        let arc = state_env.clone();
        let env = arc.guard().await;
        let reader = env.reader()?;

        // get the dna
        let dna_file = conductor_handle
            .get_dna(id.dna_hash())
            .await
            .ok_or(CellError::DnaMissing)?;

        let conductor_api = CellConductorApi::new(conductor_handle, id.clone());

        // run genesis
        let workspace = GenesisWorkspace::new(&reader, &env)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;
        let workflow = GenesisWorkflow::new(
            conductor_api,
            dna_file,
            id.agent_pubkey().clone(),
            membrane_proof,
        );

        run_workflow(state_env.clone(), workflow, workspace)
            .await
            .map_err(Box::new)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;
        Ok(state_env)
    }

    fn dna_hash(&self) -> &DnaHash {
        &self.id.dna_hash()
    }

    #[allow(unused)]
    fn agent_pubkey(&self) -> &AgentPubKey {
        &self.id.agent_pubkey()
    }

    pub fn id(&self) -> &CellId {
        &self.id
    }

    /// Entry point for incoming messages from the network that need to be handled
    pub async fn handle_network_message(&self, _msg: Todo) -> CellResult<Option<Todo>> {
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
    pub async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        let arc = self.state_env();
        let env = arc.guard().await;
        let reader = env.reader()?;
        let workspace = InvokeZomeWorkspace::new(&reader, &env)?;
        let workflow = InvokeZomeWorkflow {
            ribosome: self.get_ribosome().await?,
            invocation,
        };
        Ok(run_workflow(self.state_env().clone(), workflow, workspace)
            .await
            .map_err(Box::new)?)
    }

    pub async fn cleanup(self) -> CellResult<()> {
        let path = self.state_env.path().clone();
        // Remove db from global map
        // Delete directory
        self.state_env
            .remove()
            .await
            .map_err(|e| CellError::Cleanup(e.to_string(), path))?;
        Ok(())
    }

    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) async fn get_ribosome(&self) -> CellResult<WasmRibosome> {
        match self.conductor_api.get_dna(self.dna_hash()).await {
            Some(dna) => Ok(WasmRibosome::new(dna)),
            None => Err(CellError::DnaMissing),
        }
    }

    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) fn state_env(&self) -> &EnvironmentWrite {
        &self.state_env
    }
}

////////////////////////////////////////////////////////////////////////////////////
// The following is a sketch from the skunkworx phase, and can probably be removed

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Todo) -> Result<(), NetError>;
}

#[allow(dead_code)]
/// TODO - this is a shim until we need a real NetError
enum NetError {}
