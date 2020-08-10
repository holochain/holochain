//! A Cell is an "instance" of Holochain DNA.
//!
//! It combines an AgentPubKey with a Dna to create a SourceChain, upon which
//! Elements can be added. A constructed Cell is guaranteed to have a valid
//! SourceChain which has already undergone Genesis.

use super::manager::ManagedTaskAdd;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::{spawn_queue_consumer_tasks, InitialQueueTriggers};
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::ribosome::ZomeCallInvocationResponse;
use crate::core::state::workspace::Workspace;
use crate::{
    conductor::{api::CellConductorApi, cell::error::CellResult},
    core::ribosome::{guest_callback::init::InitResult, wasm_ribosome::WasmRibosome},
    core::{
        state::{
            dht_op_integration::IntegratedDhtOpsBuf,
            element_buf::ElementBuf,
            metadata::{LinkMetaKey, MetadataBuf, MetadataBufT},
            source_chain::SourceChainBuf,
        },
        workflow::{
            call_zome_workflow, error::WorkflowError, genesis_workflow::genesis_workflow,
            initialize_zomes_workflow, integrate_dht_ops_workflow::IntegrateDhtOpsWorkspace,
            CallZomeWorkflowArgs, CallZomeWorkspace, GenesisWorkflowArgs, GenesisWorkspace,
            InitializeZomesWorkflowArgs, InitializeZomesWorkspace, ZomeCallInvocationResult,
        },
    },
};
use error::{AuthorityDataError, CellError};
use fallible_iterator::FallibleIterator;
use futures::future::FutureExt;
use hash_type::AnyDht;
use holo_hash::*;
use holochain_keystore::{KeystoreSender, Signature};
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{
    db::GetDb,
    env::{EnvironmentKind, EnvironmentWrite, ReadManager},
};
use holochain_types::{
    autonomic::AutonomicProcess,
    cell::CellId,
    element::{GetElementResponse, WireElement},
    link::{GetLinksResponse, WireLinkMetaKey},
    metadata::{MetadataSet, TimedHeaderHash},
    Timestamp,
};
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::header::{LinkAdd, LinkRemove};
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    hash::{Hash, Hasher},
    path::Path,
};
use tokio::sync;
use tracing::*;

mod authority;

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
    queue_triggers: InitialQueueTriggers,
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
        mut holochain_p2p_cell: holochain_p2p::HolochainP2pCell,
        managed_task_add_sender: sync::mpsc::Sender<ManagedTaskAdd>,
        managed_task_stop_broadcaster: sync::broadcast::Sender<()>,
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
            holochain_p2p_cell.join().await?;
            let queue_triggers = spawn_queue_consumer_tasks(
                &state_env,
                holochain_p2p_cell.clone(),
                managed_task_add_sender,
                managed_task_stop_broadcaster,
            )
            .await;

            Ok(Self {
                id,
                conductor_api,
                state_env,
                holochain_p2p_cell,
                queue_triggers,
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
        let args = GenesisWorkflowArgs::new(dna_file, id.agent_pubkey().clone(), membrane_proof);

        genesis_workflow(workspace, state_env.clone().into(), conductor_api, args)
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
                to_agent,
                zome_name,
                fn_name,
                cap,
                respond,
                request,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_call_remote(to_agent, zome_name, fn_name, cap, request)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            Publish {
                span,
                respond,
                from_agent,
                request_validation_receipt,
                dht_hash,
                ops,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_publish(from_agent, request_validation_receipt, dht_hash, ops)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetValidationPackage { span, respond, .. } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_get_validation_package()
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            Get {
                span,
                respond,
                dht_hash,
                options,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_get(dht_hash, options)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetMeta {
                span,
                respond,
                dht_hash,
                options,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_get_meta(dht_hash, options)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetLinks {
                span,
                respond,
                link_key,
                options,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_get_links(link_key, options)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            ValidationReceiptReceived {
                span,
                respond,
                receipt,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_validation_receipt(receipt)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            FetchOpHashesForConstraints {
                span,
                respond,
                dht_arc,
                since,
                until,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_fetch_op_hashes_for_constraints(dht_arc, since, until)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            FetchOpHashData {
                span,
                respond,
                op_hashes,
                ..
            } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_fetch_op_hash_data(op_hashes)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            SignNetworkData { span, respond, .. } => {
                let _g = Span::enter(&span);
                let res = self
                    .handle_sign_network_data()
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
        }
        Ok(())
    }

    #[instrument(skip(self, _request_validation_receipt, _dht_hash, ops))]
    /// we are receiving a "publish" event from the network
    async fn handle_publish(
        &self,
        _from_agent: AgentPubKey,
        _request_validation_receipt: bool,
        _dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> CellResult<()> {
        /////////////////////////////////////////////////////////////
        // FIXME - We are temporarily just integrating everything...
        //         Really this should go to validation first!
        //         Everything below this line is throwaway code.
        /////////////////////////////////////////////////////////////

        // set up our workspace
        let env_ref = self.state_env.guard().await;
        let reader = env_ref.reader().expect("Could not create LMDB reader");
        let mut workspace =
            IntegrateDhtOpsWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");

        // add incoming ops to the integration queue transaction
        for (hash, op) in ops {
            let iqv = crate::core::state::dht_op_integration::IntegrationLimboValue {
                validation_status: holochain_types::validate::ValidationStatus::Valid,
                op,
            };
            if !workspace.op_exists(&hash)? {
                workspace.integration_limbo.put(hash, iqv)?;
            }
        }

        // commit our transaction
        let writer: crate::core::queue_consumer::OneshotWriter = self.state_env.clone().into();

        writer
            .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
            .await?;

        // trigger integration of queued ops
        self.queue_triggers.integrate_dht_ops.clone().trigger();

        Ok(())
    }

    /// a remote node is attempting to retreive a validation package
    async fn handle_get_validation_package(&self) -> CellResult<()> {
        unimplemented!()
    }

    #[instrument(skip(self, options))]
    /// a remote node is asking us for entry data
    async fn handle_get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<GetElementResponse> {
        // TODO: Later we will need more get types but for now
        // we can just have these defaults depending on whether or not
        // the hash is an entry or header.
        // In the future we should use GetOptions to choose which get to run.
        let r = match *dht_hash.hash_type() {
            AnyDht::Entry(et) => self.handle_get_entry(dht_hash.retype(et), options).await,
            AnyDht::Header => {
                self.handle_get_element(dht_hash.retype(hash_type::Header))
                    .await
            }
        };
        if let Err(e) = &r {
            error!(msg = "Error handling a get", ?e, agent = ?self.id.agent_pubkey());
        }
        r
    }

    #[instrument(skip(self, options))]
    async fn handle_get_entry(
        &self,
        hash: EntryHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<GetElementResponse> {
        let state_env = self.state_env.clone();
        authority::handle_get_entry(state_env, hash, options).await
    }

    async fn handle_get_element(&self, hash: HeaderHash) -> CellResult<GetElementResponse> {
        // Get the vaults
        let env_ref = self.state_env.guard().await;
        let dbs = self.state_env.dbs().await;
        let reader = env_ref.reader()?;
        let element_vault = ElementBuf::vault(&reader, &dbs, false)?;
        let meta_vault = MetadataBuf::vault(&reader, &dbs)?;

        // Look for a delete on the header and collect it
        let deleted = meta_vault.get_deletes_on_header(hash.clone())?.next()?;
        let deleted = match deleted {
            Some(delete_header) => {
                let delete = delete_header.header_hash;
                match element_vault.get_header(&delete).await? {
                    Some(delete) => Some(delete.try_into().map_err(AuthorityDataError::from)?),
                    None => {
                        return Err(AuthorityDataError::missing_data(delete));
                    }
                }
            }
            None => None,
        };

        // Get the actual header and return it with proof of deleted if there is any
        let r = element_vault
            .get_element(&hash)
            .await?
            .map(|e| WireElement::from_element(e, deleted))
            .map(Box::new);

        Ok(GetElementResponse::GetHeader(r))
    }

    /// a remote node is asking us for metadata
    async fn handle_get_meta(
        &self,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: holochain_p2p::event::GetMetaOptions,
    ) -> CellResult<MetadataSet> {
        unimplemented!()
    }

    #[instrument(skip(self, _options))]
    /// a remote node is asking us for links
    // TODO: Right now we are returning all the full headers
    // We could probably send some smaller types instead of the full headers
    // if we are careful.
    async fn handle_get_links(
        &self,
        link_key: WireLinkMetaKey,
        _options: holochain_p2p::event::GetLinksOptions,
    ) -> CellResult<GetLinksResponse> {
        // Get the vaults
        let env_ref = self.state_env.guard().await;
        let dbs = self.state_env.dbs().await;
        let reader = env_ref.reader()?;
        let element_vault = ElementBuf::vault(&reader, &dbs, false)?;
        let meta_vault = MetadataBuf::vault(&reader, &dbs)?;
        debug!(id = ?self.id());

        let links = meta_vault
            .get_links_all(&LinkMetaKey::from(&link_key))?
            .map(|link_add| {
                // Collect the link removes on this link add
                let link_removes = meta_vault
                    .get_link_removes_on_link_add(link_add.link_add_hash.clone())?
                    .collect::<BTreeSet<_>>()?;
                // Create timed header hash
                let link_add = TimedHeaderHash {
                    timestamp: link_add.timestamp,
                    header_hash: link_add.link_add_hash,
                };
                // Return all link removes with this link add
                Ok((link_add, link_removes))
            })
            .collect::<BTreeMap<_, _>>()?;

        // Get the headers from the element stores
        let mut result_adds: Vec<(LinkAdd, Signature)> = Vec::with_capacity(links.len());
        let mut result_removes: Vec<(LinkRemove, Signature)> = Vec::with_capacity(links.len());
        for (link_add, link_removes) in links {
            if let Some(link_add) = element_vault.get_header(&link_add.header_hash).await? {
                for link_remove in link_removes {
                    if let Some(link_remove) =
                        element_vault.get_header(&link_remove.header_hash).await?
                    {
                        let (h, s) = link_remove.into_header_and_signature();
                        let h = h
                            .into_content()
                            .try_into()
                            .map_err(AuthorityDataError::from)?;
                        result_removes.push((h, s));
                    }
                }
                let (h, s) = link_add.into_header_and_signature();
                let h = h
                    .into_content()
                    .try_into()
                    .map_err(AuthorityDataError::from)?;
                result_adds.push((h, s));
            }
        }

        // Return the links
        Ok(GetLinksResponse {
            link_adds: result_adds,
            link_removes: result_removes,
        })
    }

    /// a remote agent is sending us a validation receipt.
    async fn handle_validation_receipt(&self, _receipt: SerializedBytes) -> CellResult<()> {
        unimplemented!()
    }

    /// the network module is requesting a list of dht op hashes
    async fn handle_fetch_op_hashes_for_constraints(
        &self,
        dht_arc: holochain_p2p::dht_arc::DhtArc,
        since: Timestamp,
        until: Timestamp,
    ) -> CellResult<Vec<DhtOpHash>> {
        let env_ref = self.state_env.guard().await;
        let reader = env_ref.reader()?;
        let integrated_dht_ops = IntegratedDhtOpsBuf::new(&reader, &env_ref)?;
        let result: Vec<DhtOpHash> = integrated_dht_ops
            .query(Some(since), Some(until), Some(dht_arc))?
            .map(|(k, _)| Ok(k))
            .collect()?;
        Ok(result)
    }

    /// the network module is requesting the content for dht ops
    async fn handle_fetch_op_hash_data(
        &self,
        op_hashes: Vec<holo_hash::DhtOpHash>,
    ) -> CellResult<
        Vec<(
            holo_hash::AnyDhtHash,
            holo_hash::DhtOpHash,
            holochain_types::dht_op::DhtOp,
        )>,
    > {
        let env_ref = self.state_env.guard().await;
        let reader = env_ref.reader()?;
        let integrated_dht_ops = IntegratedDhtOpsBuf::new(&reader, &env_ref)?;
        let cas = ElementBuf::vault(&reader, &env_ref, false)?;
        let mut out = vec![];
        for op_hash in op_hashes {
            let val = integrated_dht_ops.get(&op_hash)?;
            if let Some(val) = val {
                let full_op =
                    crate::core::workflow::produce_dht_ops_workflow::dht_op_light::light_to_op(
                        val.op, &cas,
                    )
                    .await?;
                let basis = full_op.dht_basis().await;
                out.push((basis, op_hash, full_op));
            }
        }
        Ok(out)
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

    /// a remote agent is attempting a "call_remote" on this cell.
    async fn handle_call_remote(
        &self,
        provenance: AgentPubKey,
        zome_name: ZomeName,
        fn_name: String,
        cap: CapSecret,
        payload: SerializedBytes,
    ) -> CellResult<SerializedBytes> {
        let invocation = ZomeCallInvocation {
            cell_id: self.id.clone(),
            zome_name: zome_name.clone(),
            cap,
            payload: HostInput::new(payload),
            provenance,
            fn_name,
        };
        // double ? because
        // - ConductorApiResult
        // - ZomeCallInvocationResult
        match self.call_zome(invocation).await?? {
            ZomeCallInvocationResponse::ZomeApiFn(guest_output) => Ok(guest_output.into_inner()),
            //currently unreachable
            //_ => Err(RibosomeError::ZomeFnNotExists(zome_name, "A remote zome call failed in a way that should not be possible.".into()))?,
        }
    }

    /// Function called by the Conductor
    #[instrument(skip(self))]
    pub async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
    ) -> CellResult<ZomeCallInvocationResult> {
        // Check if init has run if not run it
        self.check_or_run_zome_init().await?;

        let arc = self.state_env();
        let keystore = arc.keystore().clone();
        let env = arc.guard().await;
        let reader = env.reader()?;
        let workspace = CallZomeWorkspace::new(&reader, &env)?;

        let args = CallZomeWorkflowArgs {
            ribosome: self.get_ribosome().await?,
            invocation,
        };
        Ok(call_zome_workflow(
            workspace,
            self.holochain_p2p_cell.clone(),
            keystore,
            self.state_env().clone().into(),
            args,
            self.queue_triggers.produce_dht_ops.clone(),
        )
        .await
        .map_err(Box::new)?)
    }

    /// Check if each Zome's init callback has been run, and if not, run it.
    async fn check_or_run_zome_init(&self) -> CellResult<()> {
        // If not run it
        let state_env = self.state_env.clone();
        let keystore = state_env.keystore().clone();
        let id = self.id.clone();
        let conductor_api = self.conductor_api.clone();
        let env_ref = state_env.guard().await;
        let reader = env_ref.reader()?;
        // Create the workspace
        let workspace = CallZomeWorkspace::new(&reader, &env_ref)
            .map_err(WorkflowError::from)
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
        let init_result = initialize_zomes_workflow(
            workspace,
            self.holochain_p2p_cell.clone(),
            keystore,
            state_env.clone().into(),
            args,
        )
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

#[cfg(test)]
mod test;
