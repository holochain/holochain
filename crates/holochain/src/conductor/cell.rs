//! A Cell is an "instance" of Holochain DNA.
//!
//! It combines an AgentPubKey with a Dna to create a SourceChain, upon which
//! ChainElements can be added. A constructed Cell is guaranteed to have a valid
//! SourceChain which has already undergone Genesis.

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::spawn_queue_consumer_tasks;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::state::workspace::Workspace;
use crate::{
    conductor::{
        api::{error::ConductorApiResult, CellConductorApi},
        cell::error::CellResult,
    },
    core::ribosome::{guest_callback::init::InitResult, wasm_ribosome::WasmRibosome},
    core::{
        state::source_chain::SourceChainBuf,
        workflow::{
            error::WorkflowRunError, genesis_workflow::genesis_workflow, initialize_zomes_workflow,
            run_workflow, GenesisWorkflowArgs, GenesisWorkspace, InitializeZomesWorkflowArgs,
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
use holochain_types::{autonomic::AutonomicProcess, cell::CellId, prelude::Todo};
use std::{
    hash::{Hash, Hasher},
    path::Path,
};
use tracing::*;

#[allow(missing_docs)]
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
/// A Cell is guaranteed to contain a Source Chain which has undergone
/// Genesis.
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
    holochain_p2p_cell: holochain_p2p::HolochainP2pCell,
}

impl Cell {
    /// Constructor for a Cell. The SourceChain will be created, and genesis
    /// will be run if necessary. A Cell will not be created if the SourceChain
    /// is not ready to be used.
    pub async fn create<P: AsRef<Path>>(
        id: CellId,
        conductor_handle: ConductorHandle,
        env_path: P,
        keystore: KeystoreSender,
        holochain_p2p_cell: holochain_p2p::HolochainP2pCell,
    ) -> CellResult<Self> {
        let conductor_api = CellConductorApi::new(conductor_handle.clone(), id.clone());

        // get the environment
        let state_env = EnvironmentWrite::new(
            env_path.as_ref(),
            EnvironmentKind::Cell(id.clone()),
            keystore,
        )?;

        // check if genesis has been run
        let has_genesis = {
            // check if genesis ran on source chain buf
            let env_ref = state_env.guard().await;
            let reader = env_ref.reader()?;
            SourceChainBuf::new(&reader, &env_ref)?.has_genesis()
        };

        if has_genesis {
            // TODO: store these triggers somewhere so they can be hooked up
            // to InvokeCallZome and HandleGossip workflows
            let triggers = spawn_queue_consumer_tasks(state_env.clone());

            Ok(Self {
                id,
                conductor_api,
                state_env,
                holochain_p2p_cell,
            })
        } else {
            Err(CellError::CellWithoutGenesis(id))
        }
    }

    /// Performs the Genesis workflow the Cell, ensuring that its initial
    /// elements are committed. This is a prerequisite for any other interaction
    /// with the SourceChain
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
        let args = GenesisWorkflowArgs::new(
            conductor_api,
            dna_file,
            id.agent_pubkey().clone(),
            membrane_proof,
        );

        genesis_workflow(workspace, state_env.clone().into(), args)
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

    /// Accessor
    pub fn id(&self) -> &CellId {
        &self.id
    }

    /// Access a network sender that is partially applied to this cell's DnaHash/AgentPubKey
    pub fn holochain_p2p_cell(&self) -> &holochain_p2p::HolochainP2pCell {
        &self.holochain_p2p_cell
    }

    /// Entry point for incoming messages from the network that need to be handled
    pub async fn handle_holochain_p2p_event(
        &self,
        evt: holochain_p2p::event::HolochainP2pEvent,
    ) -> CellResult<()> {
        use holochain_p2p::event::HolochainP2pEvent::*;
        match evt {
            CallRemote {
                span,
                respond,
                request,
                ..
            } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_call_remote(request)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            Publish {
                span,
                respond,
                from_agent,
                request_validation_receipt,
                entry_hash,
                ops,
                ..
            } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_publish(from_agent, request_validation_receipt, entry_hash, ops)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            GetValidationPackage { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_get_validation_package()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            Get { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_get()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            GetLinks { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_get_links()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            ValidationReceiptReceived {
                span,
                respond,
                receipt,
                ..
            } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_validation_receipt(receipt)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            ListDhtOpHashes { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_list_dht_op_hashes()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            FetchDhtOps { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_fetch_dht_ops()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
            SignNetworkData { span, respond, .. } => {
                let _g = span.enter();
                let _ = respond(
                    self.handle_sign_network_data()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other),
                );
            }
        }
        Ok(())
    }

    /// a remote agent is attempting a "call_remote" on this cell.
    async fn handle_call_remote(&self, request: SerializedBytes) -> CellResult<SerializedBytes> {
        // This is a stub call remote handler that just
        // echoes whatever is sent to it.
        // TODO - Implement the real call_remote handler.
        Ok(request)
    }

    /// we are receiving a "publish" event from the network
    async fn handle_publish(
        &self,
        _from_agent: AgentPubKey,
        _request_validation_receipt: bool,
        _entry_hash: holochain_types::composite_hash::AnyDhtHash,
        _ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> CellResult<()> {
        unimplemented!()
    }

    /// a remote node is attempting to retreive a validation package
    async fn handle_get_validation_package(&self) -> CellResult<()> {
        unimplemented!()
    }

    /// a remote node is asking us for entry data
    async fn handle_get(&self) -> CellResult<()> {
        unimplemented!()
    }

    /// a remote node is asking us for links
    async fn handle_get_links(&self) -> CellResult<()> {
        unimplemented!()
    }

    /// a remote agent is sending us a validation receipt.
    async fn handle_validation_receipt(&self, _receipt: SerializedBytes) -> CellResult<()> {
        unimplemented!()
    }

    /// the network module is requesting a list of dht op hashes
    async fn handle_list_dht_op_hashes(&self) -> CellResult<()> {
        unimplemented!()
    }

    /// the network module is requesting the content for dht ops
    async fn handle_fetch_dht_ops(&self) -> CellResult<()> {
        unimplemented!()
    }

    /// the network module would like this cell/agent to sign some data
    async fn handle_sign_network_data(&self) -> CellResult<holochain_keystore::Signature> {
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
        // Check if init has run if not run it
        self.check_or_run_zome_init().await?;

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

    /// Check if each Zome's init callback has been run, and if not, run it.
    async fn check_or_run_zome_init(&self) -> CellResult<()> {
        // If not run it
        let state_env = self.state_env.clone();
        let id = self.id.clone();
        let conductor_api = self.conductor_api.clone();
        let env_ref = state_env.guard().await;
        let reader = env_ref.reader()?;
        // Create the workspace
        let workspace = InvokeZomeWorkspace::new(&reader, &env_ref)
            .map_err(WorkflowRunError::from)
            .map_err(Box::new)?;
        let workspace = InitializeZomesWorkspace(workspace);

        // Check if initialization has run
        if workspace.0.source_chain.has_initialized() {
            return Ok(());
        }
        trace!("running init");

        // get the dna
        let dna_file = conductor_api
            .get_dna(id.dna_hash())
            .await
            .ok_or(CellError::DnaMissing)?;
        let dna_def = dna_file.dna().clone();

        // Get the ribosome
        let ribosome = WasmRibosome::new(dna_file);

        // Run the workflow
        let args = InitializeZomesWorkflowArgs { dna_def, ribosome };
        let init_result = initialize_zomes_workflow(workspace, state_env.clone().into(), args)
            .await
            .map_err(Box::new)?;
        trace!(?init_result);
        match init_result {
            InitResult::Pass => (),
            r => return Err(CellError::InitFailed(r)),
        }
        Ok(())
    }

    /// Delete all data associated with this Cell by deleting the associated
    /// LMDB environment. Completely reverses Cell creation.
    pub async fn destroy(self) -> CellResult<()> {
        let path = self.state_env.path().clone();
        // Remove db from global map
        // Delete directory
        self.state_env
            .remove()
            .await
            .map_err(|e| CellError::Cleanup(e.to_string(), path))?;
        Ok(())
    }

    /// Instantiate a Ribosome for use by this Cell's workflows
    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) async fn get_ribosome(&self) -> CellResult<WasmRibosome> {
        match self.conductor_api.get_dna(self.dna_hash()).await {
            Some(dna) => Ok(WasmRibosome::new(dna)),
            None => Err(CellError::DnaMissing),
        }
    }

    /// Accessor for the LMDB environment backing this Cell
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
