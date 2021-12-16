//! A Cell is an "instance" of Holochain DNA.
//!
//! It combines an AgentPubKey with a Dna to create a SourceChain, upon which
//! Elements can be added. A constructed Cell is guaranteed to have a valid
//! SourceChain which has already undergone Genesis.

use super::api::ZomeCall;
use super::interface::SignalBroadcaster;
use super::manager::ManagedTaskAdd;
use super::space::Space;
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
use crate::core::workflow::countersigning_workflow::countersigning_success;
use crate::core::workflow::genesis_workflow::genesis_workflow;
use crate::core::workflow::initialize_zomes_workflow;
use crate::core::workflow::CallZomeWorkflowArgs;
use crate::core::workflow::GenesisWorkflowArgs;
use crate::core::workflow::GenesisWorkspace;
use crate::core::workflow::InitializeZomesWorkflowArgs;
use crate::core::workflow::ZomeCallResult;
use crate::{conductor::api::error::ConductorApiError, core::ribosome::RibosomeT};
use error::CellError;
use futures::future::FutureExt;
use hash_type::AnyDht;
use holo_hash::*;
use holochain_cascade::authority;
use holochain_cascade::Cascade;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::prelude::*;
use holochain_state::schedule::live_scheduled_fns;
use holochain_types::prelude::*;
use rusqlite::OptionalExtension;
use rusqlite::Transaction;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use tokio::sync;
use tracing::*;
use tracing_futures::Instrument;

pub const INIT_MUTEX_TIMEOUT_SECS: u64 = 30;

mod validation_package;

#[allow(missing_docs)]
pub mod error;

#[cfg(test)]
mod gossip_test;
#[cfg(todo_redo_old_tests)]
mod op_query_test;

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
pub struct Cell<Api = CellConductorApi, P2pCell = holochain_p2p::HolochainP2pDna>
where
    Api: CellConductorApiT,
    P2pCell: holochain_p2p::HolochainP2pDnaT,
{
    id: CellId,
    conductor_api: Api,
    conductor_handle: ConductorHandle,
    space: Space,
    holochain_p2p_cell: P2pCell,
    queue_triggers: QueueTriggers,
    init_mutex: tokio::sync::Mutex<()>,
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
        space: Space,
        holochain_p2p_cell: holochain_p2p::HolochainP2pDna,
        managed_task_add_sender: sync::mpsc::Sender<ManagedTaskAdd>,
        managed_task_stop_broadcaster: sync::broadcast::Sender<()>,
    ) -> CellResult<(Self, InitialQueueTriggers)> {
        let conductor_api = CellConductorApi::new(conductor_handle.clone(), id.clone());

        // check if genesis has been run
        let has_genesis = {
            // check if genesis ran.
            GenesisWorkspace::new(space.authored_env.clone(), space.dht_env.clone())?
                .has_genesis(id.agent_pubkey().clone())
                .await?
        };

        if has_genesis {
            let (queue_triggers, initial_queue_triggers) = spawn_queue_consumer_tasks(
                id.clone(),
                holochain_p2p_cell.clone(),
                &space,
                conductor_handle.clone(),
                managed_task_add_sender,
                managed_task_stop_broadcaster,
            )
            .await;

            Ok((
                Self {
                    id,
                    conductor_api,
                    conductor_handle,
                    space,
                    holochain_p2p_cell,
                    queue_triggers,
                    init_mutex: Default::default(),
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
    pub async fn genesis<Ribosome>(
        id: CellId,
        conductor_handle: ConductorHandle,
        authored_env: DbWrite<DbKindAuthored>,
        dht_env: DbWrite<DbKindDht>,
        ribosome: Ribosome,
        membrane_proof: Option<SerializedBytes>,
    ) -> CellResult<()>
    where
        Ribosome: RibosomeT + Send + 'static,
    {
        // get the dna
        let dna_file = conductor_handle
            .get_dna(id.dna_hash())
            .ok_or_else(|| DnaError::DnaMissing(id.dna_hash().to_owned()))?;

        let conductor_api = CellConductorApi::new(conductor_handle.clone(), id.clone());

        // run genesis
        let workspace = GenesisWorkspace::new(authored_env, dht_env)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;

        let args = GenesisWorkflowArgs::new(
            dna_file,
            id.agent_pubkey().clone(),
            membrane_proof,
            ribosome,
        );

        genesis_workflow(workspace, conductor_api, args)
            .await
            .map_err(Box::new)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;

        if let Some(trigger) = conductor_handle
            .get_queue_consumer_workflows()
            .integration_trigger(Arc::new(id.dna_hash().clone()))
        {
            trigger.trigger();
        }
        Ok(())
    }

    fn dna_hash(&self) -> &DnaHash {
        self.id.dna_hash()
    }

    #[allow(unused)]
    fn agent_pubkey(&self) -> &AgentPubKey {
        self.id.agent_pubkey()
    }

    /// Accessor
    pub fn id(&self) -> &CellId {
        &self.id
    }

    /// Access a network sender that is partially applied to this cell's DnaHash/AgentPubKey
    pub fn holochain_p2p_dna(&self) -> &holochain_p2p::HolochainP2pDna {
        &self.holochain_p2p_cell
    }

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor_api.signal_broadcaster().await
    }

    pub(super) async fn delete_all_ephemeral_scheduled_fns(self: Arc<Self>) -> CellResult<()> {
        let author = self.id.agent_pubkey().clone();
        Ok(self
            .space
            .authored_env
            .async_commit(move |txn: &mut Transaction| {
                delete_all_ephemeral_scheduled_fns(txn, &author)
            })
            .await?)
    }

    pub(super) async fn dispatch_scheduled_fns(self: Arc<Self>) {
        let now = Timestamp::now();
        let author = self.id.agent_pubkey().clone();
        let lives = self
            .space
            .authored_env
            .async_commit(move |txn: &mut Transaction| {
                // Rescheduling should not fail as the data in the database
                // should be valid schedules only.
                reschedule_expired(txn, now, &author)?;
                let lives = live_scheduled_fns(txn, now, &author);
                // We know what to run so we can delete the ephemerals.
                if lives.is_ok() {
                    // Failing to delete should rollback this attempt.
                    delete_live_ephemeral_scheduled_fns(txn, now, &author)?;
                }
                lives
            })
            .await;

        match lives {
            // Cannot proceed if we don't know what to run.
            Err(e) => {
                error!("{}", e.to_string());
            }
            Ok(lives) => {
                let mut tasks = vec![];
                for (scheduled_fn, schedule) in &lives {
                    // Failing to encode a schedule should never happen.
                    // If it does log the error and bail.
                    let payload = match ExternIO::encode(schedule) {
                        Ok(payload) => payload,
                        Err(e) => {
                            error!("{}", e.to_string());
                            continue;
                        }
                    };
                    let invocation = ZomeCall {
                        cell_id: self.id.clone(),
                        zome_name: scheduled_fn.zome_name().clone(),
                        cap_secret: None,
                        payload,
                        provenance: self.id.agent_pubkey().clone(),
                        fn_name: scheduled_fn.fn_name().clone(),
                    };
                    tasks.push(self.call_zome(invocation, None));
                }
                let results: Vec<CellResult<ZomeCallResult>> =
                    futures::future::join_all(tasks).await;

                let author = self.id.agent_pubkey().clone();
                // We don't do anything with errors in here.
                let _ = self
                    .space
                    .authored_env
                    .async_commit(move |txn: &mut Transaction| {
                        for ((scheduled_fn, _), result) in lives.iter().zip(results.iter()) {
                            match result {
                                Ok(Ok(ZomeCallResponse::Ok(extern_io))) => {
                                    let next_schedule: Schedule = match extern_io.decode() {
                                        Ok(Some(v)) => v,
                                        Ok(None) => {
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("{}", e.to_string());
                                            continue;
                                        }
                                    };
                                    // Ignore errors so that failing to schedule
                                    // one function doesn't error others.
                                    // For example if a zome returns a bad cron.
                                    if let Err(e) = schedule_fn(
                                        txn,
                                        &author,
                                        scheduled_fn.clone(),
                                        Some(next_schedule),
                                        now,
                                    ) {
                                        error!("{}", e.to_string());
                                        continue;
                                    }
                                }
                                errorish => error!("{:?}", errorish),
                            }
                        }
                        Result::<(), DatabaseError>::Ok(())
                    })
                    .await;
            }
        }
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
            | QueryGossipAgents { .. }
            | QueryOpHashes { .. }
            | QueryAgentInfoSignedNearBasis { .. }
            | QueryPeerDensity { .. }
            | Publish { .. }
            | PutMetricDatum { .. }
            | FetchOpData { .. }
            | QueryMetrics { .. } => {
                // These events are aggregated over a set of cells, so need to be handled at the conductor level.
                unreachable!()
            }
            CallRemote {
                span_context: _,
                from_agent,
                zome_name,
                fn_name,
                cap_secret,
                respond,
                payload,
                ..
            } => {
                async {
                    let res = self
                        .handle_call_remote(from_agent, zome_name, fn_name, cap_secret, payload)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("call_remote"))
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
                        .await
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
                        .await
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
                // We got a receipt so we must be connected to the network
                // and should reset the publish back off loop to its minimum.
                self.queue_triggers.publish_dht_ops.reset_back_off();
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
            CountersigningAuthorityResponse {
                respond,
                signed_headers,
                ..
            } => {
                async {
                    let res = self
                        .handle_countersigning_authority_response(signed_headers)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_countersigning_response"))
                .await;
            }
        }
        Ok(())
    }

    #[instrument(skip(self, signed_headers))]
    /// we are receiving a response from a countersigning authority
    async fn handle_countersigning_authority_response(
        &self,
        signed_headers: Vec<SignedHeader>,
    ) -> CellResult<()> {
        Ok(countersigning_success(
            self.space.authored_env.clone(),
            self.space.dht_env.clone(),
            &self.holochain_p2p_cell,
            self.id.agent_pubkey().clone(),
            signed_headers,
            self.queue_triggers.clone(),
            self.conductor_api.signal_broadcaster().await,
        )
        .await
        .map_err(Box::new)?)
    }

    #[instrument(skip(self))]
    /// a remote node is attempting to retrieve a validation package
    #[tracing::instrument(skip(self), level = "trace")]
    async fn handle_get_validation_package(
        &self,
        header_hash: HeaderHash,
    ) -> CellResult<ValidationPackageResponse> {
        let env: DbRead<DbKindDht> = self.dht_env().clone().into();

        // Get the header
        let mut cascade = Cascade::empty().with_dht(env.clone());
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
                self.space.authored_env.clone().into(),
                self.dht_env().clone().into(),
                self.space.cache.clone(),
                &ribosome,
                &(*self.conductor_handle),
                &self.holochain_p2p_cell,
            )
            .await
        } else {
            validation_package::get_as_authority(
                header,
                env,
                &ribosome.dna_file,
                self.conductor_handle.as_ref(),
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
    ) -> CellResult<WireOps> {
        debug!("handling get");
        // TODO: Later we will need more get types but for now
        // we can just have these defaults depending on whether or not
        // the hash is an entry or header.
        // In the future we should use GetOptions to choose which get to run.
        let mut r = match *dht_hash.hash_type() {
            AnyDht::Entry => self
                .handle_get_entry(dht_hash.into(), options)
                .await
                .map(WireOps::Entry),
            AnyDht::Header => self
                .handle_get_element(dht_hash.into(), options)
                .await
                .map(WireOps::Element),
        };
        if let Err(e) = &mut r {
            error!(msg = "Error handling a get", ?e, agent = ?self.id.agent_pubkey());
        }
        r
    }

    #[instrument(skip(self, options))]
    async fn handle_get_entry(
        &self,
        hash: EntryHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<WireEntryOps> {
        let env = self.space.dht_env.clone();
        authority::handle_get_entry(env.into(), hash, options)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn handle_get_element(
        &self,
        hash: HeaderHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<WireElementOps> {
        let env = self.space.dht_env.clone();
        authority::handle_get_element(env.into(), hash, options)
            .await
            .map_err(Into::into)
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
    async fn handle_get_links(
        &self,
        link_key: WireLinkKey,
        options: holochain_p2p::event::GetLinksOptions,
    ) -> CellResult<WireLinkOps> {
        debug!(id = ?self.id());
        let env = self.space.dht_env.clone();
        authority::handle_get_links(env.into(), link_key, options)
            .await
            .map_err(Into::into)
    }

    #[instrument(skip(self, options))]
    async fn handle_get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: holochain_p2p::event::GetActivityOptions,
    ) -> CellResult<AgentActivityResponse<HeaderHash>> {
        let env = self.space.dht_env.clone();
        authority::handle_get_agent_activity(env.into(), agent, query, options)
            .await
            .map_err(Into::into)
    }

    /// a remote agent is sending us a validation receipt.
    #[tracing::instrument(skip(self, receipt))]
    async fn handle_validation_receipt(&self, receipt: SerializedBytes) -> CellResult<()> {
        let receipt: SignedValidationReceipt = receipt.try_into()?;
        tracing::debug!(from = ?receipt.receipt.validators, to = ?self.id.agent_pubkey(), hash = ?receipt.receipt.dht_op_hash);

        // Get the header for this op so we can check the entry type.
        let hash = receipt.receipt.dht_op_hash.clone();
        let header: Option<SignedHeader> = self
            .space
            .authored_env
            .async_reader(move |txn| {
                let h: Option<Vec<u8>> = txn
                    .query_row(
                        "SELECT Header.blob as header_blob
                    FROM DhtOp
                    JOIN Header ON Header.hash = DhtOp.header_hash
                    WHERE DhtOp.hash = :hash",
                        named_params! {
                            ":hash": hash,
                        },
                        |row| row.get("header_blob"),
                    )
                    .optional()?;
                match h {
                    Some(h) => from_blob(h),
                    None => Ok(None),
                }
            })
            .await?;

        // If the header has an app entry type get the entry def
        // from the conductor.
        let required_receipt_count = match header.as_ref().and_then(|h| h.0.entry_type()) {
            Some(EntryType::App(entry_type)) => {
                let zome_index = u8::from(entry_type.zome_id()) as usize;
                let dna_file = self.conductor_api.get_this_dna().map_err(Box::new)?;
                let zome = dna_file.dna().zomes.get(zome_index).map(|(_, z)| z.clone());
                match zome {
                    Some(zome) => self
                        .conductor_api
                        .get_entry_def(&EntryDefBufferKey::new(zome, entry_type.id()))
                        .map(|e| u8::from(e.required_validations)),
                    None => None,
                }
            }
            _ => None,
        };

        // If no required receipt count was found then fallback to the default.
        let required_validation_count = required_receipt_count.unwrap_or(
            crate::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE,
        );

        self.space
            .dht_env
            .async_commit(move |txn| {
                // Get the current count for this dhtop.
                let receipt_count: usize = txn.query_row(
                    "SELECT COUNT(rowid) FROM ValidationReceipt WHERE op_hash = :op_hash",
                    named_params! {
                        ":op_hash": receipt.receipt.dht_op_hash,
                    },
                    |row| row.get(0),
                )?;

                // If we have enough receipts then set receipts to complete.
                if receipt_count >= required_validation_count as usize {
                    set_receipts_complete(txn, &receipt.receipt.dht_op_hash, true)?;
                }

                // Add to receipts db
                validation_receipts::add_if_unique(txn, receipt)
            })
            .await?;

        Ok(())
    }

    /// the network module would like this cell/agent to sign some data
    #[tracing::instrument(skip(self))]
    async fn handle_sign_network_data(&self) -> CellResult<Signature> {
        Ok([0; 64].into())
    }

    #[instrument(skip(self, from_agent, fn_name, cap_secret, payload))]
    /// a remote agent is attempting a "call_remote" on this cell.
    async fn handle_call_remote(
        &self,
        from_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
    ) -> CellResult<SerializedBytes> {
        let invocation = ZomeCall {
            cell_id: self.id.clone(),
            zome_name,
            cap_secret,
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
        workspace_lock: Option<SourceChainWorkspace>,
    ) -> CellResult<ZomeCallResult> {
        // Check if init has run if not run it
        self.check_or_run_zome_init().await?;

        let keystore = self.conductor_api.keystore().clone();

        // If there is no existing zome call then this is the root zome call
        let is_root_zome_call = workspace_lock.is_none();
        let workspace_lock = match workspace_lock {
            Some(l) => l,
            None => {
                SourceChainWorkspace::new(
                    self.authored_env().clone(),
                    self.dht_env().clone(),
                    self.cache().clone(),
                    keystore.clone(),
                    self.id.agent_pubkey().clone(),
                )
                .await?
            }
        };

        let conductor_handle = self.conductor_handle.clone();
        let signal_tx = self.signal_broadcaster().await;
        let ribosome = self.get_ribosome().await?;
        let invocation =
            ZomeCallInvocation::from_interface_call(self.conductor_api.clone(), call).await;

        let args = CallZomeWorkflowArgs {
            cell_id: self.id.clone(),
            ribosome,
            invocation,
            signal_tx,
            conductor_handle,
            is_root_zome_call,
        };
        Ok(call_zome_workflow(
            workspace_lock,
            self.holochain_p2p_cell.clone(),
            keystore,
            args,
            self.queue_triggers.publish_dht_ops.clone(),
            self.queue_triggers.integrate_dht_ops.clone(),
        )
        .await
        .map_err(Box::new)?)
    }

    /// Check if each Zome's init callback has been run, and if not, run it.
    #[tracing::instrument(skip(self))]
    async fn check_or_run_zome_init(&self) -> CellResult<()> {
        // Ensure that only one init check is run at a time
        let _guard = tokio::time::timeout(
            std::time::Duration::from_secs(INIT_MUTEX_TIMEOUT_SECS),
            self.init_mutex.lock(),
        )
        .await
        .map_err(|_| CellError::InitTimeout)?;

        // If not run it
        let keystore = self.conductor_api.keystore().clone();
        let id = self.id.clone();
        let conductor_handle = self.conductor_handle.clone();
        // Create the workspace
        let workspace = SourceChainWorkspace::new(
            self.authored_env().clone(),
            self.dht_env().clone(),
            self.cache().clone(),
            keystore.clone(),
            id.agent_pubkey().clone(),
        )
        .await?;

        // Check if initialization has run
        if workspace.source_chain().has_initialized()? {
            return Ok(());
        }
        trace!("running init");

        // get the dna
        let dna_file = conductor_handle
            .get_dna(id.dna_hash())
            .ok_or_else(|| DnaError::DnaMissing(id.dna_hash().to_owned()))?;
        let dna_def = dna_file.dna_def().clone();

        // Get the ribosome
        let ribosome = RealRibosome::new(dna_file);

        // Run the workflow
        let args = InitializeZomesWorkflowArgs {
            dna_def,
            ribosome,
            conductor_handle,
        };
        let init_result =
            initialize_zomes_workflow(workspace, self.holochain_p2p_cell.clone(), keystore, args)
                .await
                .map_err(Box::new)?;
        trace!(?init_result);
        match init_result {
            InitResult::Pass => {}
            r => return Err(CellError::InitFailed(r)),
        }
        Ok(())
    }

    /// Clean up long-running managed tasks.
    //
    // FIXME: this should ensure that the long-running managed tasks,
    //        i.e. the queue consumers, are stopped. Currently, they
    //        will continue running because we have no way to target a specific
    //        Cell's tasks for shutdown.
    //
    //        Consider using a separate TaskManager for each Cell, so that all
    //        of a Cell's tasks can be shut down at once. Perhaps the Conductor
    //        TaskManager can have these Cell TaskManagers as children.
    //        [ B-04176 ]
    pub async fn cleanup(&self) -> CellResult<()> {
        use holochain_p2p::HolochainP2pDnaT;
        self.holochain_p2p_dna()
            .leave(self.id.agent_pubkey().clone())
            .await?;
        tracing::info!("Cell removed, but cleanup is not yet fully implemented.");
        Ok(())
    }

    /// Instantiate a Ribosome for use by this Cell's workflows
    pub(crate) async fn get_ribosome(&self) -> CellResult<RealRibosome> {
        match self.conductor_api.get_dna(self.dna_hash()) {
            Some(dna) => Ok(RealRibosome::new(dna)),
            None => Err(DnaError::DnaMissing(self.dna_hash().to_owned()).into()),
        }
    }

    /// Accessor for the authored database backing this Cell
    pub(crate) fn authored_env(&self) -> &DbWrite<DbKindAuthored> {
        &self.space.authored_env
    }

    /// Accessor for the authored database backing this Cell
    pub(crate) fn dht_env(&self) -> &DbWrite<DbKindDht> {
        &self.space.dht_env
    }

    pub(crate) fn cache(&self) -> &DbWrite<DbKindCache> {
        &self.space.cache
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Get the triggers for the cell
    /// Useful for testing when you want to
    /// Cause workflows to trigger
    pub(crate) fn triggers(&self) -> &QueueTriggers {
        &self.queue_triggers
    }
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell").field("id", &self.id()).finish()
    }
}
