//! A Cell is an "instance" of Holochain DNA.
//!
//! It combines an AgentPubKey with a Dna to create a SourceChain, upon which
//! Elements can be added. A constructed Cell is guaranteed to have a valid
//! SourceChain which has already undergone Genesis.

use super::api::ZomeCall;
use super::interface::SignalBroadcaster;
use super::manager::ManagedTaskAdd;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::cell::error::CellResult;
use crate::conductor::entry_def_store::get_entry_def_from_ids;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::spawn_queue_consumer_tasks;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::core::queue_consumer::QueueTriggers;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::call_zome_workflow;
use crate::core::workflow::error::WorkflowError;
use crate::core::workflow::genesis_workflow::genesis_workflow;
use crate::core::workflow::incoming_dht_ops_workflow::incoming_dht_ops_workflow;
use crate::core::workflow::initialize_zomes_workflow;
use crate::core::workflow::produce_dht_ops_workflow::dht_op_light::light_to_op;
use crate::core::workflow::CallZomeWorkflowArgs;
use crate::core::workflow::CallZomeWorkspace;
use crate::core::workflow::GenesisWorkflowArgs;
use crate::core::workflow::GenesisWorkspace;
use crate::core::workflow::InitializeZomesWorkflowArgs;
use crate::core::workflow::ZomeCallResult;
use call_zome_workflow::call_zome_workspace_lock::CallZomeWorkspaceLock;
use error::CellError;
use fallible_iterator::FallibleIterator;
use futures::future::FutureExt;
use hash_type::AnyDht;
use holo_hash::*;
use holochain_cascade::authority;
use holochain_lmdb::db::GetDb;
use holochain_lmdb::env::EnvironmentRead;
use holochain_lmdb::env::EnvironmentWrite;
use holochain_lmdb::env::ReadManager;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use observability::OpenSpanExt;
use std::hash::Hash;
use std::hash::Hasher;
use tokio::sync;
use tracing::*;
use tracing_futures::Instrument;
use validation_package::ValidationPackageDb;

mod validation_package;

#[allow(missing_docs)]
pub mod error;

#[cfg(test)]
mod gossip_test;

#[cfg(test)]
mod test;

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
pub struct Cell<Api = CellConductorApi, P2pCell = holochain_p2p::HolochainP2pCell>
where
    Api: CellConductorApiT,
    P2pCell: holochain_p2p::HolochainP2pCellT,
{
    id: CellId,
    conductor_api: Api,
    env: EnvironmentWrite,
    holochain_p2p_cell: P2pCell,
    queue_triggers: QueueTriggers,
}

impl Cell {
    /// Constructor for a Cell, which ensure the Cell is fully initialized
    /// before returning.
    ///
    /// If it hasn't happened already, a SourceChain will be created, and
    /// genesis will be run. If these have already happened, those steps are
    /// skipped.
    ///
    /// No Cell will be created if the SourceChain is not ready to be used.
    pub async fn create(
        id: CellId,
        conductor_handle: ConductorHandle,
        env: EnvironmentWrite,
        holochain_p2p_cell: holochain_p2p::HolochainP2pCell,
        managed_task_add_sender: sync::mpsc::Sender<ManagedTaskAdd>,
        managed_task_stop_broadcaster: sync::broadcast::Sender<()>,
    ) -> CellResult<(Self, InitialQueueTriggers)> {
        let conductor_api = CellConductorApi::new(conductor_handle.clone(), id.clone());

        // check if genesis has been run
        let has_genesis = {
            // check if genesis ran on source chain buf
            SourceChainBuf::new(env.clone().into())?.has_genesis()
        };

        if has_genesis {
            let (queue_triggers, initial_queue_triggers) = spawn_queue_consumer_tasks(
                &env,
                holochain_p2p_cell.clone(),
                conductor_api.clone(),
                managed_task_add_sender,
                managed_task_stop_broadcaster,
            )
            .await;

            Ok((
                Self {
                    id,
                    conductor_api,
                    env,
                    holochain_p2p_cell,
                    queue_triggers,
                },
                initial_queue_triggers,
            ))
        } else {
            Err(CellError::CellWithoutGenesis(id))
        }
    }

    /// Performs the Genesis workflow the Cell, ensuring that its initial
    /// elements are committed. This is a prerequisite for any other interaction
    /// with the SourceChain
    pub async fn genesis(
        id: CellId,
        conductor_handle: ConductorHandle,
        cell_env: EnvironmentWrite,
        membrane_proof: Option<SerializedBytes>,
    ) -> CellResult<()> {
        // get the dna
        let dna_file = conductor_handle
            .get_dna(id.dna_hash())
            .await
            .ok_or_else(|| DnaError::DnaMissing(id.dna_hash().to_owned()))?;

        let conductor_api = CellConductorApi::new(conductor_handle, id.clone());

        // run genesis
        let workspace = GenesisWorkspace::new(cell_env.clone().into())
            .await
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;
        let args = GenesisWorkflowArgs::new(dna_file, id.agent_pubkey().clone(), membrane_proof);

        genesis_workflow(workspace, cell_env.clone().into(), conductor_api, args)
            .await
            .map_err(Box::new)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;
        Ok(())
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

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor_api.signal_broadcaster().await
    }

    #[instrument(skip(self, evt))]
    /// Entry point for incoming messages from the network that need to be handled
    pub async fn handle_holochain_p2p_event(
        &self,
        evt: holochain_p2p::event::HolochainP2pEvent,
    ) -> CellResult<()> {
        use holochain_p2p::event::HolochainP2pEvent::*;
        match evt {
            PutAgentInfoSigned { .. }
            | GetAgentInfoSigned { .. }
            | QueryAgentInfoSigned { .. }
            | AgentInfoByBasis { .. } => {
                // PutAgentInfoSigned needs to be handled at the conductor level where the p2p
                // store lives.
                unreachable!()
            }
            CallRemote {
                span_context: _,
                from_agent,
                zome_name,
                fn_name,
                cap,
                respond,
                payload,
                ..
            } => {
                async {
                    let res = self
                        .handle_call_remote(from_agent, zome_name, fn_name, cap, payload)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("call_remote"))
                .await;
            }
            Publish {
                span_context,
                respond,
                from_agent,
                request_validation_receipt,
                dht_hash,
                ops,
                ..
            } => {
                async {
                    tracing::Span::set_current_context(span_context);
                    let res = self
                        .handle_publish(from_agent, request_validation_receipt, dht_hash, ops)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_publish"))
                .await;
            }
            GetValidationPackage {
                span_context: _,
                respond,
                header_hash,
                ..
            } => {
                async {
                    let res = self
                        .handle_get_validation_package(header_hash)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_get_validation_package"))
                .await;
            }
            Get {
                span_context: _,
                respond,
                dht_hash,
                options,
                ..
            } => {
                async {
                    let res = self
                        .handle_get(dht_hash, options)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_get"))
                .await;
            }
            GetMeta {
                span_context: _,
                respond,
                dht_hash,
                options,
                ..
            } => {
                async {
                    let res = self
                        .handle_get_meta(dht_hash, options)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_get_meta"))
                .await;
            }
            GetLinks {
                span_context: _,
                respond,
                link_key,
                options,
                ..
            } => {
                async {
                    let res = self
                        .handle_get_links(link_key, options)
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_get_links"))
                .await;
            }
            GetAgentActivity {
                span_context: _,
                respond,
                agent,
                query,
                options,
                ..
            } => {
                async {
                    let res = self
                        .handle_get_agent_activity(agent, query, options)
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_get_agent_activity"))
                .await;
            }
            ValidationReceiptReceived {
                span_context: _,
                respond,
                receipt,
                ..
            } => {
                async {
                    let res = self
                        .handle_validation_receipt(receipt)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_validation_receipt_received"))
                .await;
            }
            FetchOpHashesForConstraints {
                span_context: _,
                respond,
                dht_arc,
                since,
                until,
                ..
            } => {
                async {
                    let res = self
                        .handle_fetch_op_hashes_for_constraints(dht_arc, since, until)
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_fetch_op_hashes_for_constraints"))
                .await;
            }
            FetchOpHashData {
                span_context: _,
                respond,
                op_hashes,
                ..
            } => {
                async {
                    let res = self
                        .handle_fetch_op_hash_data(op_hashes)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_fetch_op_hash_data"))
                .await;
            }
            SignNetworkData {
                span_context: _,
                respond,
                ..
            } => {
                async {
                    let res = self
                        .handle_sign_network_data()
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_sign_network_data"))
                .await;
            }
        }
        Ok(())
    }

    #[instrument(skip(self, request_validation_receipt, _dht_hash, ops))]
    /// we are receiving a "publish" event from the network
    async fn handle_publish(
        &self,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        _dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> CellResult<()> {
        incoming_dht_ops_workflow(
            &self.env,
            self.queue_triggers.sys_validation.clone(),
            ops,
            Some(from_agent),
            request_validation_receipt,
        )
        .await
        .map_err(Box::new)
        .map_err(ConductorApiError::from)
        .map_err(Box::new)?;
        Ok(())
    }

    #[instrument(skip(self))]
    /// a remote node is attempting to retrieve a validation package
    #[tracing::instrument(skip(self), level = "trace")]
    async fn handle_get_validation_package(
        &self,
        header_hash: HeaderHash,
    ) -> CellResult<ValidationPackageResponse> {
        let env: EnvironmentRead = self.env.clone().into();

        // Get the header
        let databases = ValidationPackageDb::create(env.clone())?;
        let mut cascade = databases.cascade();
        let header = match cascade
            .retrieve_header(header_hash, Default::default())
            .await?
        {
            Some(shh) => shh.into_header_and_signature().0,
            None => return Ok(None.into()),
        };

        let ribosome = self.get_ribosome().await?;

        // This agent is the author so get the validation package from the source chain
        if header.author() == self.id.agent_pubkey() {
            validation_package::get_as_author(
                header,
                env,
                &ribosome,
                &self.conductor_api,
                &self.holochain_p2p_cell,
            )
            .await
        } else {
            validation_package::get_as_authority(
                header,
                env,
                &ribosome.dna_file,
                &self.conductor_api,
            )
            .await
        }
    }

    #[instrument(skip(self, options))]
    /// a remote node is asking us for entry data
    async fn handle_get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<GetElementResponse> {
        debug!("handling get");
        // TODO: Later we will need more get types but for now
        // we can just have these defaults depending on whether or not
        // the hash is an entry or header.
        // In the future we should use GetOptions to choose which get to run.
        let r = match *dht_hash.hash_type() {
            AnyDht::Entry => self.handle_get_entry(dht_hash.into(), options).await,
            AnyDht::Header => self.handle_get_element(dht_hash.into()).await,
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
        let env = self.env.clone();
        authority::handle_get_entry(env, hash, options).map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn handle_get_element(&self, hash: HeaderHash) -> CellResult<GetElementResponse> {
        let env = self.env.clone();
        authority::handle_get_element(env, hash).map_err(Into::into)
    }

    #[instrument(skip(self, _dht_hash, _options))]
    /// a remote node is asking us for metadata
    async fn handle_get_meta(
        &self,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: holochain_p2p::event::GetMetaOptions,
    ) -> CellResult<MetadataSet> {
        unimplemented!()
    }

    #[instrument(skip(self, options))]
    /// a remote node is asking us for links
    // TODO: Right now we are returning all the full headers
    // We could probably send some smaller types instead of the full headers
    // if we are careful.
    fn handle_get_links(
        &self,
        link_key: WireLinkMetaKey,
        options: holochain_p2p::event::GetLinksOptions,
    ) -> CellResult<GetLinksResponse> {
        debug!(id = ?self.id());
        let env = self.env.clone();
        authority::handle_get_links(env.into(), link_key, options).map_err(Into::into)
    }

    #[instrument(skip(self, options))]
    fn handle_get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: holochain_p2p::event::GetActivityOptions,
    ) -> CellResult<AgentActivityResponse> {
        let env = self.env.clone();
        authority::handle_get_agent_activity(env.into(), agent, query, options).map_err(Into::into)
    }

    /// a remote agent is sending us a validation receipt.
    #[tracing::instrument(skip(self))]
    async fn handle_validation_receipt(&self, receipt: SerializedBytes) -> CellResult<()> {
        let receipt: SignedValidationReceipt = receipt.try_into()?;

        // Add to authored
        let db = self.env.get_db(&*AUTHORED_DHT_OPS)?;
        let mut authored_dht_ops: AuthoredDhtOpsStore =
            KvBufFresh::new(self.env.clone().into(), db);
        match authored_dht_ops.get(&receipt.receipt.dht_op_hash)? {
            Some(mut auth) => {
                auth.receipt_count += 1;
                authored_dht_ops.put(receipt.receipt.dht_op_hash.clone(), auth)?;
            }
            None => {
                warn!(
                    "Got receipt {:?} but it's missing from authored db so throwing receipt away",
                    receipt
                );
                return Err(CellError::OpMissingForReceipt(receipt.receipt.dht_op_hash));
            }
        }

        // Add to receipts db
        let mut receipts_db = ValidationReceiptsBuf::new(&self.env)?;
        receipts_db.add_if_unique(receipt)?;

        // Write to db
        self.env.with_commit(|w| {
            authored_dht_ops.flush_to_txn_ref(w)?;
            receipts_db.flush_to_txn_ref(w)?;
            DatabaseResult::Ok(())
        })?;

        Ok(())
    }

    #[instrument(skip(self, dht_arc, since, until))]
    /// the network module is requesting a list of dht op hashes
    fn handle_fetch_op_hashes_for_constraints(
        &self,
        dht_arc: holochain_p2p::dht_arc::DhtArc,
        since: Timestamp,
        until: Timestamp,
    ) -> CellResult<Vec<DhtOpHash>> {
        let env_ref = self.env.guard();
        let reader = env_ref.reader()?;
        let integrated_dht_ops = IntegratedDhtOpsBuf::new(self.env().clone().into())?;
        let result: Vec<DhtOpHash> = integrated_dht_ops
            .query(&reader, Some(since), Some(until), Some(dht_arc))?
            .map(|(k, _)| Ok(k))
            .collect()?;
        Ok(result)
    }

    #[instrument(skip(self, op_hashes))]
    /// The network module is requesting the content for dht ops
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
        let integrated_dht_ops = IntegratedDhtOpsBuf::new(self.env().clone().into())?;
        let mut out = vec![];
        for op_hash in op_hashes {
            let val = integrated_dht_ops.get(&op_hash)?;
            if let Some(val) = val {
                let full_op = match &val.validation_status {
                    ValidationStatus::Valid => {
                        let cas = ElementBuf::vault(self.env.clone().into(), false)?;
                        light_to_op(val.op, &cas)?
                    }
                    ValidationStatus::Rejected => {
                        let cas = ElementBuf::rejected(self.env.clone().into())?;
                        light_to_op(val.op, &cas)?
                    }
                    ValidationStatus::Abandoned => todo!("Add when abandoned store is added"),
                };
                let basis = full_op.dht_basis();
                out.push((basis, op_hash, full_op));
            }
        }
        Ok(out)
    }

    /// the network module would like this cell/agent to sign some data
    #[tracing::instrument(skip(self))]
    async fn handle_sign_network_data(&self) -> CellResult<Signature> {
        Ok([0; 64].into())
    }

    /// When the Conductor determines that it's time to execute some [AutonomicProcess],
    /// whether scheduled or through an [AutonomicCue], this function gets called
    #[tracing::instrument(skip(self, process))]
    pub async fn handle_autonomic_process(&self, process: AutonomicProcess) -> CellResult<()> {
        match process {
            AutonomicProcess::SlowHeal => unimplemented!(),
            AutonomicProcess::HealthCheck => unimplemented!(),
        }
    }

    #[instrument(skip(self, from_agent, fn_name, cap, payload))]
    /// a remote agent is attempting a "call_remote" on this cell.
    async fn handle_call_remote(
        &self,
        from_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> CellResult<SerializedBytes> {
        let invocation = ZomeCall {
            cell_id: self.id.clone(),
            zome_name,
            cap,
            payload,
            provenance: from_agent,
            fn_name,
        };
        // double ? because
        // - ConductorApiResult
        // - ZomeCallResult
        Ok(self.call_zome(invocation, None).await??.try_into()?)
    }

    /// Function called by the Conductor
    #[instrument(skip(self, call, workspace_lock))]
    pub async fn call_zome(
        &self,
        call: ZomeCall,
        workspace_lock: Option<CallZomeWorkspaceLock>,
    ) -> CellResult<ZomeCallResult> {
        // Check if init has run if not run it
        self.check_or_run_zome_init().await?;

        let arc = self.env();
        let keystore = arc.keystore().clone();

        // If there is no existing zome call then this is the root zome call
        let is_root_zome_call = workspace_lock.is_none();
        let workspace_lock = match workspace_lock {
            Some(l) => l,
            None => CallZomeWorkspaceLock::new(CallZomeWorkspace::new(arc.clone().into())?),
        };

        let conductor_api = self.conductor_api.clone();
        let signal_tx = self.signal_broadcaster().await;
        let ribosome = self.get_ribosome().await?;
        let invocation = ZomeCallInvocation::from_interface_call(conductor_api.clone(), call).await;

        let args = CallZomeWorkflowArgs {
            ribosome,
            invocation,
            conductor_api,
            signal_tx,
            is_root_zome_call,
        };
        Ok(call_zome_workflow(
            workspace_lock,
            self.holochain_p2p_cell.clone(),
            keystore,
            arc.clone().into(),
            args,
            self.queue_triggers.produce_dht_ops.clone(),
        )
        .await
        .map_err(Box::new)?)
    }

    /// Check if each Zome's init callback has been run, and if not, run it.
    #[tracing::instrument(skip(self))]
    async fn check_or_run_zome_init(&self) -> CellResult<()> {
        // If not run it
        let env = self.env.clone();
        let keystore = env.keystore().clone();
        let id = self.id.clone();
        let conductor_api = self.conductor_api.clone();
        // Create the workspace
        let workspace = CallZomeWorkspace::new(self.env().clone().into())
            .map_err(WorkflowError::from)
            .map_err(Box::new)?;

        // Check if initialization has run
        if workspace.source_chain.has_initialized() {
            return Ok(());
        }
        trace!("running init");

        // get the dna
        let dna_file = conductor_api
            .get_dna(id.dna_hash())
            .await
            .ok_or_else(|| DnaError::DnaMissing(id.dna_hash().to_owned()))?;
        let dna_def = dna_file.dna_def().clone();

        // Get the ribosome
        let ribosome = RealRibosome::new(dna_file);

        // Run the workflow
        let args = InitializeZomesWorkflowArgs { dna_def, ribosome };
        let init_result = initialize_zomes_workflow(
            workspace,
            self.holochain_p2p_cell.clone(),
            keystore,
            env.clone().into(),
            args,
        )
        .await
        .map_err(Box::new)?;
        trace!(?init_result);
        match init_result {
            InitResult::Pass => {}
            r => return Err(CellError::InitFailed(r)),
        }
        Ok(())
    }

    /// Delete all data associated with this Cell by deleting the associated
    /// LMDB environment. Completely reverses Cell creation.
    #[tracing::instrument(skip(self))]
    pub async fn destroy(self) -> CellResult<()> {
        let path = self.env.path().clone();
        // Remove db from global map
        // Delete directory
        self.env
            .remove()
            .await
            .map_err(|e| CellError::Cleanup(e.to_string(), path))?;
        Ok(())
    }

    /// Instantiate a Ribosome for use by this Cell's workflows
    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) async fn get_ribosome(&self) -> CellResult<RealRibosome> {
        match self.conductor_api.get_dna(self.dna_hash()).await {
            Some(dna) => Ok(RealRibosome::new(dna)),
            None => Err(DnaError::DnaMissing(self.dna_hash().to_owned()).into()),
        }
    }

    /// Accessor for the LMDB environment backing this Cell
    // TODO: reevaluate once Workflows are fully implemented (after B-01567)
    pub(crate) fn env(&self) -> &EnvironmentWrite {
        &self.env
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Get the triggers for the cell
    /// Useful for testing when you want to
    /// Cause workflows to trigger
    pub(crate) fn triggers(&self) -> &QueueTriggers {
        &self.queue_triggers
    }
}
