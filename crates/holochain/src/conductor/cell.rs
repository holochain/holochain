//! A Cell is an "instance" of Holochain DNA.
//!
//! It combines an AgentPubKey with a Dna to create a SourceChain, upon which
//! Records can be added. A constructed Cell is guaranteed to have a valid
//! SourceChain which has already undergone Genesis.

use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use futures::future::FutureExt;
use holochain_serialized_bytes::SerializedBytes;
use rusqlite::OptionalExtension;
use rusqlite::Transaction;
use tokio::sync::broadcast;
use tracing::*;
use tracing_futures::Instrument;

use error::CellError;
use holo_hash::*;
use holochain_cascade::authority;
use holochain_chc::ChcImpl;
use holochain_conductor_api::ZomeCall;
use holochain_nonce::fresh_nonce;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::HolochainP2pDna;
use holochain_sqlite::prelude::*;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::prelude::*;
use holochain_state::schedule::live_scheduled_fns;
use holochain_types::db_cache::DhtDbQueryCache;

use crate::conductor::api::CellConductorApi;
use crate::conductor::cell::error::CellResult;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::core::queue_consumer::QueueTriggers;
use crate::core::queue_consumer::{spawn_queue_consumer_tasks, TriggerSender};
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::call_zome_workflow;
use crate::core::workflow::countersigning_workflow::countersigning_success;
use crate::core::workflow::genesis_workflow::genesis_workflow;
use crate::core::workflow::initialize_zomes_workflow;
use crate::core::workflow::witnessing_workflow::receive_incoming_countersigning_ops;
use crate::core::workflow::CallZomeWorkflowArgs;
use crate::core::workflow::GenesisWorkflowArgs;
use crate::core::workflow::GenesisWorkspace;
use crate::core::workflow::InitializeZomesWorkflowArgs;
use crate::core::workflow::ZomeCallResult;
use crate::{conductor::api::error::ConductorApiError, core::ribosome::RibosomeT};

use super::api::CellConductorHandle;
use super::space::Space;
use super::ConductorHandle;

pub const INIT_MUTEX_TIMEOUT_SECS: u64 = 30;

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
/// The [`Conductor`](super::Conductor) manages a collection of Cells, and will call functions
/// on the Cell when a Conductor API method is called (either a
/// [`CellConductorApi`](super::api::CellConductorApi) or an [`AppInterfaceApi`](super::api::AppInterfaceApi))
pub struct Cell {
    id: CellId,
    conductor_api: CellConductorHandle,
    // NOTE: this got snuck in here, the original purpose was that the Cell would have limited access to
    // the full Conductor via `CellConductorHandle`. As it stands, it's redundant to have both, but it
    // may make it easier to a cleanup of the Conductor monolith later if we don't completely remove
    // the encapsulation of CellConductorHandle, even though the encapsulation is not complete.
    conductor_handle: ConductorHandle,
    space: Space,
    holochain_p2p_cell: HolochainP2pDna,
    queue_triggers: QueueTriggers,
    signal_tx: broadcast::Sender<Signal>,
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
        holochain_p2p_cell: HolochainP2pDna,
        signal_tx: broadcast::Sender<Signal>,
    ) -> CellResult<(Self, InitialQueueTriggers)> {
        let conductor_api = Arc::new(CellConductorApi::new(conductor_handle.clone(), id.clone()));
        let authored_db = space.get_or_create_authored_db(id.agent_pubkey().clone())?;

        // check if genesis has been run
        let has_genesis = {
            // check if genesis ran.
            GenesisWorkspace::new(authored_db.clone(), space.dht_db.clone())?
                .has_genesis(id.agent_pubkey().clone())
                .await?
        };

        if has_genesis {
            let (queue_triggers, initial_queue_triggers) = spawn_queue_consumer_tasks(
                id.clone(),
                holochain_p2p_cell.clone(),
                &space,
                conductor_handle.clone(),
            )
            .await
            .map_err(Box::new)?;

            Ok((
                Self {
                    id,
                    conductor_api,
                    conductor_handle,
                    space,
                    holochain_p2p_cell,
                    queue_triggers,
                    signal_tx,
                    init_mutex: Default::default(),
                },
                initial_queue_triggers,
            ))
        } else {
            Err(CellError::CellWithoutGenesis(id))
        }
    }

    /// Performs the Genesis workflow for the Cell, ensuring that its initial
    /// records are committed. This is a prerequisite for any other interaction
    /// with the SourceChain
    #[allow(clippy::too_many_arguments)]
    pub async fn genesis<Ribosome>(
        cell_id: CellId,
        conductor_handle: ConductorHandle,
        authored_db: DbWrite<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        ribosome: Ribosome,
        membrane_proof: Option<MembraneProof>,
        chc: Option<ChcImpl>,
    ) -> CellResult<()>
    where
        Ribosome: RibosomeT + 'static,
    {
        // get the dna
        let dna_file = conductor_handle
            .get_dna_file(cell_id.dna_hash())
            .ok_or_else(|| DnaError::DnaMissing(cell_id.dna_hash().to_owned()))?;

        let conductor_api = CellConductorApi::new(conductor_handle.clone(), cell_id.clone());

        // run genesis
        let workspace = GenesisWorkspace::new(authored_db, dht_db)
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;

        // exit early if genesis has already run
        if workspace
            .has_genesis(cell_id.agent_pubkey().clone())
            .await?
        {
            return Ok(());
        }

        let args = GenesisWorkflowArgs::new(
            dna_file,
            cell_id.agent_pubkey().clone(),
            membrane_proof,
            ribosome,
            dht_db_cache,
            chc,
        );

        genesis_workflow(workspace, conductor_api, args)
            .await
            .map_err(ConductorApiError::from)
            .map_err(Box::new)?;

        if let Some(trigger) = conductor_handle
            .get_queue_consumer_workflows()
            .integration_trigger(Arc::new(cell_id.dna_hash().clone()))
        {
            trigger.trigger(&"genesis");
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

    pub(super) async fn dispatch_scheduled_fns(self: Arc<Self>, now: Timestamp) {
        let authored_db = match self.get_or_create_authored_db() {
            Ok(db) => db,
            Err(e) => {
                error!(
                    "error getting authored db, cannot dispatch scheduled functions: {:?}",
                    e
                );
                return;
            }
        };

        let author = self.id.agent_pubkey().clone();
        let live_fns = authored_db
            .write_async(move |txn: &mut Transaction| {
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

        match live_fns {
            // Cannot proceed if we don't know what to run.
            Err(e) => {
                error!("error calling scheduled fn: {:?}", e);
            }
            Ok(live_fns) => {
                let mut tasks = vec![];
                for (scheduled_fn, schedule) in &live_fns {
                    // Failing to encode a schedule should never happen.
                    // If it does log the error and bail.
                    let payload = match ExternIO::encode(schedule) {
                        Ok(payload) => payload,
                        Err(e) => {
                            error!(
                                "error encoding scheduled fn: {:?} error: {:?}",
                                scheduled_fn, e
                            );
                            continue;
                        }
                    };
                    let provenance = self.id.agent_pubkey().clone();
                    let (nonce, expires_at) = match fresh_nonce(now) {
                        Ok(v) => v,
                        Err(e) => {
                            error!(
                                "error creating nonce for fn: {:?} error: {:?}",
                                scheduled_fn, e
                            );
                            continue;
                        }
                    };
                    let unsigned_zome_call = ZomeCallUnsigned {
                        provenance,
                        cell_id: self.id.clone(),
                        zome_name: scheduled_fn.zome_name().clone(),
                        fn_name: scheduled_fn.fn_name().clone(),
                        cap_secret: None,
                        payload,
                        nonce,
                        expires_at,
                    };

                    tasks.push(
                        self.call_zome(
                            match ZomeCall::try_from_unsigned_zome_call(
                                self.conductor_handle.keystore(),
                                unsigned_zome_call,
                            )
                                .await
                            {
                                Ok(zome_call) => zome_call,
                                Err(e) => {
                                    error!("scheduled zome call error in try_from_unsigned_zome_call: {:?}", e);
                                    continue;
                                }
                            },
                            None,
                        ),
                    );
                }
                let results: Vec<CellResult<ZomeCallResult>> =
                    futures::future::join_all(tasks).await;

                let author = self.id.agent_pubkey().clone();
                // We don't do anything with errors in here.
                let _ = authored_db
                    .write_async(move |txn: &mut Transaction| {
                        for ((scheduled_fn, _), result) in live_fns.iter().zip(results.iter()) {
                            match result {
                                Ok(Ok(ZomeCallResponse::Ok(extern_io))) => {
                                    let next_schedule: Schedule = match extern_io.decode() {
                                        Ok(Some(v)) => v,
                                        Ok(None) => {
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("scheduled zome call error in ExternIO::decode: {:?}", e);
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
                                        error!("scheduled zome call error in schedule_fn: {:?}", e);
                                        continue;
                                    }
                                }
                                errorish => error!("scheduled zome call error: {:?}", errorish),
                            }
                        }
                        Result::<(), DatabaseError>::Ok(())
                    })
                    .await;
            }
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, evt)))]
    /// Entry point for incoming messages from the network that need to be handled
    //
    // TODO: when we had CellStatus to track whether a cell had joined the network or not,
    // we would disallow zome calls for cells which had not joined. If we want that behavior,
    // we can do that check at the time of this function call, rather than at the time of trying
    // to access the Cell itself, as it was previously done.
    pub async fn handle_holochain_p2p_event(
        &self,
        evt: holochain_p2p::event::HolochainP2pEvent,
    ) -> CellResult<()> {
        use holochain_p2p::event::HolochainP2pEvent::*;
        match evt {
            PutAgentInfoSigned { .. }
            | QueryAgentInfoSigned { .. }
            | QueryGossipAgents { .. }
            | QueryOpHashes { .. }
            | QueryAgentInfoSignedNearBasis { .. }
            | QueryPeerDensity { .. }
            | Publish { .. }
            | FetchOpData { .. } => {
                // These events are aggregated over a set of cells, so need to be handled at the conductor level.
                unreachable!()
            }

            CallRemote {
                span_context: _,
                from_agent,
                signature,
                zome_name,
                fn_name,
                cap_secret,
                respond,
                payload,
                nonce,
                expires_at,
                ..
            } => {
                async {
                    let res = self
                        .handle_call_remote(
                            from_agent, signature, zome_name, fn_name, cap_secret, payload, nonce,
                            expires_at,
                        )
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("call_remote"))
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

            CountLinks {
                span_context: _,
                respond,
                query,
                ..
            } => {
                async {
                    let res = self
                        .handle_count_links(query)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_count_links"))
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

            MustGetAgentActivity {
                span_context: _,
                respond,
                author,
                filter,
                ..
            } => {
                async {
                    let res = self
                        .handle_must_get_agent_activity(author, filter)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                .instrument(debug_span!("cell_handle_must_get_agent_activity"))
                .await;
            }

            ValidationReceiptsReceived {
                span_context: _,
                respond,
                receipts,
                ..
            } => {
                async {
                    let res = self
                        .handle_validation_receipts(receipts)
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

            CountersigningSessionNegotiation {
                respond, message, ..
            } => {
                async {
                    let res = self
                        .handle_countersigning_session_negotiation(message)
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

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    /// we are receiving a response from a countersigning authority
    async fn handle_countersigning_session_negotiation(
        &self,
        message: CountersigningSessionNegotiationMessage,
    ) -> CellResult<()> {
        match message {
            CountersigningSessionNegotiationMessage::EnzymePush(chain_op) => {
                let ops = vec![*chain_op]
                    .into_iter()
                    .map(|op| {
                        let hash = DhtOpHash::with_data_sync(&op);
                        (hash, op)
                    })
                    .collect();
                receive_incoming_countersigning_ops(
                    ops,
                    &self.space.witnessing_workspace,
                    self.queue_triggers.witnessing.clone(),
                )
                .map_err(Box::new)?;
                Ok(())
            }
            CountersigningSessionNegotiationMessage::AuthorityResponse(signed_actions) => {
                countersigning_success(
                    self.space.clone(),
                    self.id.agent_pubkey().clone(),
                    signed_actions,
                    self.queue_triggers.countersigning.clone(),
                )
                .await;

                Ok(())
            }
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    /// a remote node is asking us for entry data
    async fn handle_get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<WireOps> {
        debug!("handling get");
        // TODO: Later we will need more get types but for now
        // we can just have these defaults depending on whether or not
        // the hash is an entry or action.
        // In the future we should use GetOptions to choose which get to run.
        let mut r = match dht_hash.into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => self
                .handle_get_entry(hash, options)
                .await
                .map(WireOps::Entry),
            AnyDhtHashPrimitive::Action(hash) => self
                .handle_get_record(hash, options)
                .await
                .map(WireOps::Record),
        };
        if let Err(e) = &mut r {
            error!(msg = "Error handling a get", ?e, agent = ?self.id.agent_pubkey());
        }
        r
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    async fn handle_get_entry(
        &self,
        hash: EntryHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<WireEntryOps> {
        let db = self.space.dht_db.clone();
        authority::handle_get_entry(db.into(), hash, options)
            .await
            .map_err(Into::into)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn handle_get_record(
        &self,
        hash: ActionHash,
        options: holochain_p2p::event::GetOptions,
    ) -> CellResult<WireRecordOps> {
        let db = self.space.dht_db.clone();
        authority::handle_get_record(db.into(), hash, options)
            .await
            .map_err(Into::into)
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self, _dht_hash, _options))
    )]
    /// a remote node is asking us for metadata
    async fn handle_get_meta(
        &self,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: holochain_p2p::event::GetMetaOptions,
    ) -> CellResult<MetadataSet> {
        unimplemented!()
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    /// a remote node is asking us for links
    // TODO: Right now we are returning all the full actions
    // We could probably send some smaller types instead of the full actions
    // if we are careful.
    async fn handle_get_links(
        &self,
        link_key: WireLinkKey,
        options: holochain_p2p::event::GetLinksOptions,
    ) -> CellResult<WireLinkOps> {
        debug!(id = ?self.id());
        let db = self.space.dht_db.clone();
        authority::handle_get_links(db.into(), link_key, options)
            .await
            .map_err(Into::into)
    }

    /// a remote node is asking us to count links
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn handle_count_links(&self, query: WireLinkQuery) -> CellResult<CountLinksResponse> {
        let db = self.space.dht_db.clone();
        Ok(CountLinksResponse::new(
            authority::handle_get_links_query(db.into(), query)
                .await?
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<_>>(),
        ))
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    async fn handle_get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: holochain_p2p::event::GetActivityOptions,
    ) -> CellResult<AgentActivityResponse> {
        let db = self.space.dht_db.clone();
        authority::handle_get_agent_activity(db.into(), agent, query, options)
            .await
            .map_err(Into::into)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn handle_must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> CellResult<MustGetAgentActivityResponse> {
        let db = self.space.dht_db.clone();
        authority::handle_must_get_agent_activity(db.into(), author, filter)
            .await
            .map_err(Into::into)
    }

    /// A remote agent is sending us a validation receipt bundle.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, receipts)))]
    async fn handle_validation_receipts(
        &self,
        receipts: ValidationReceiptBundle,
    ) -> CellResult<()> {
        for receipt in receipts.into_iter() {
            debug!(from = ?receipt.receipt.validators, to = ?self.id.agent_pubkey(), hash = ?receipt.receipt.dht_op_hash);

            // Get the action for this op so we can check the entry type.
            let hash = receipt.receipt.dht_op_hash.clone();
            let action: Option<SignedAction> = self
                .get_or_create_authored_db()?
                .read_async(move |txn| {
                    let h: Option<Vec<u8>> = txn
                        .query_row(
                            "SELECT Action.blob as action_blob
                    FROM DhtOp
                    JOIN Action ON Action.hash = DhtOp.action_hash
                    WHERE DhtOp.hash = :hash",
                            named_params! {
                                ":hash": hash,
                            },
                            |row| row.get("action_blob"),
                        )
                        .optional()?;
                    match h {
                        Some(h) => from_blob(h),
                        None => Ok(None),
                    }
                })
                .await?;

            // If the action has an app entry type get the entry def
            // from the conductor.
            let required_receipt_count = match action.as_ref().and_then(|h| h.entry_type()) {
                Some(EntryType::App(AppEntryDef {
                    zome_index,
                    entry_index,
                    ..
                })) => {
                    let ribosome = self.conductor_api.get_this_ribosome().map_err(Box::new)?;
                    let zome = ribosome.get_integrity_zome(zome_index);
                    match zome {
                        Some(zome) => self
                            .conductor_api
                            .get_entry_def(&EntryDefBufferKey::new(
                                zome.into_inner().1,
                                *entry_index,
                            ))
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

            let receipt_op_hash = receipt.receipt.dht_op_hash.clone();

            let receipt_count = self
                .space
                .dht_db
                .write_async({
                    let receipt_op_hash = receipt_op_hash.clone();
                    move |txn| -> StateMutationResult<usize> {
                        // Add the new receipts to the db
                        add_if_unique(txn, receipt)?;

                        // Get the current count for this DhtOp.
                        let receipt_count: usize = txn.query_row(
                            "SELECT COUNT(rowid) FROM ValidationReceipt WHERE op_hash = :op_hash",
                            named_params! {
                                ":op_hash": receipt_op_hash,
                            },
                            |row| row.get(0),
                        )?;

                        if receipt_count >= required_validation_count as usize {
                            // If we have enough receipts then set receipts to complete.
                            //
                            // Don't fail here if this doesn't work, it's only informational. Getting
                            // the same flag set in the authored db is what will stop the publish
                            // workflow from republishing this op.
                            set_receipts_complete(txn, &receipt_op_hash, true).ok();
                        }

                        Ok(receipt_count)
                    }
                })
                .await?;

            // If we have enough receipts then set receipts to complete.
            if receipt_count >= required_validation_count as usize {
                // Note that the flag is set in the authored db because that's what the publish workflow checks to decide
                // whether to republish the op for more validation receipts.
                self.get_or_create_authored_db()?
                    .write_async(move |txn| -> StateMutationResult<()> {
                        set_receipts_complete(txn, &receipt_op_hash, true)
                    })
                    .await?;
            }
        }

        Ok(())
    }

    /// the network module would like this cell/agent to sign some data
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn handle_sign_network_data(&self) -> CellResult<Signature> {
        Ok([0; 64].into())
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self, from_agent, fn_name, cap_secret, payload))
    )]
    #[allow(clippy::too_many_arguments)]
    /// a remote agent is attempting a "call_remote" on this cell.
    async fn handle_call_remote(
        &self,
        from_agent: AgentPubKey,
        from_signature: Signature,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
        nonce: Nonce256Bits,
        expires_at: Timestamp,
    ) -> CellResult<SerializedBytes> {
        let invocation = ZomeCall {
            cell_id: self.id.clone(),
            zome_name,
            cap_secret,
            payload,
            provenance: from_agent,
            signature: from_signature,
            fn_name,
            nonce,
            expires_at,
        };
        // double ? because
        // - ConductorApiResult
        // - ZomeCallResult
        Ok(self.call_zome(invocation, None).await??.try_into()?)
    }

    /// Function called by the Conductor
    //
    // TODO: when we had CellStatus to track whether a cell had joined the network or not,
    // we would disallow zome calls for cells which had not joined. If we want that behavior,
    // we can do that check at the time of the zome call, rather than at the time of trying
    // to access the Cell itself, as it was previously done.
    pub async fn call_zome(
        &self,
        call: ZomeCall,
        workspace_lock: Option<SourceChainWorkspace>,
    ) -> CellResult<ZomeCallResult> {
        // Only check if init has run if this call is not coming from
        // an already running init call.
        if workspace_lock
            .as_ref()
            .map_or(true, |w| !w.called_from_init())
        {
            // Check if init has run if not run it
            self.check_or_run_zome_init().await?;
        }

        let keystore = self.conductor_api.keystore().clone();

        let conductor_handle = self.conductor_handle.clone();
        let ribosome = self.get_ribosome()?;
        let invocation =
            ZomeCallInvocation::try_from_interface_call(self.conductor_api.clone(), call).await?;

        let dna_def = ribosome.dna_def().as_content().clone();
        // If there is no existing zome call then this is the root zome call
        let is_root_zome_call = workspace_lock.is_none();
        let workspace_lock = match workspace_lock {
            Some(l) => l,
            None => {
                SourceChainWorkspace::new(
                    self.get_or_create_authored_db()?,
                    self.dht_db().clone(),
                    self.space.dht_query_cache.clone(),
                    self.cache().clone(),
                    keystore.clone(),
                    self.id.agent_pubkey().clone(),
                    Arc::new(dna_def),
                )
                .await?
            }
        };
        let args = CallZomeWorkflowArgs {
            cell_id: self.id.clone(),
            ribosome,
            invocation,
            signal_tx: self.signal_tx.clone(),
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
            self.queue_triggers.countersigning.clone(),
        )
        .await
        .map_err(Box::new)?)
    }

    /// Check if each Zome's init callback has been run, and if not, run it.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub(crate) async fn check_or_run_zome_init(&self) -> CellResult<()> {
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

        // get the dna
        let ribosome = self.get_ribosome()?;

        let dna_def = ribosome.dna_def().clone();

        // Create the workspace
        let workspace = SourceChainWorkspace::init_as_root(
            self.get_or_create_authored_db()?,
            self.dht_db().clone(),
            self.space.dht_query_cache.clone(),
            self.cache().clone(),
            keystore.clone(),
            id.agent_pubkey().clone(),
            Arc::new(dna_def.into_content()),
        )
        .await?;

        // Check if initialization has run
        if workspace.source_chain().zomes_initialized().await? {
            return Ok(());
        }
        trace!("running init");

        // Run the workflow
        let args = InitializeZomesWorkflowArgs {
            ribosome,
            conductor_handle,
            signal_tx: self.signal_tx.clone(),
            cell_id: self.id.clone(),
            integrate_dht_ops_trigger: self.queue_triggers.integrate_dht_ops.clone(),
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(cell_id = ?self.id())))]
    pub async fn cleanup(&self) -> CellResult<()> {
        use holochain_p2p::HolochainP2pDnaT;
        let shutdown = self
            .conductor_handle
            .task_manager()
            .stop_cell_tasks(self.id().clone())
            .map(|r| CellResult::Ok(r?))
            .in_current_span();
        let leave = self
            .holochain_p2p_dna()
            .leave(self.id.agent_pubkey().clone())
            .map(|r| CellResult::Ok(r?))
            .in_current_span();
        let (shutdown, leave) = futures::future::join(shutdown, leave).await;
        shutdown?;
        leave?;
        tracing::info!("Cell cleaned up and removed: {:?}", self.id());
        Ok(())
    }

    /// Instantiate a Ribosome for use by this Cell's workflows
    pub(crate) fn get_ribosome(&self) -> CellResult<RealRibosome> {
        Ok(self
            .conductor_handle
            .get_ribosome(self.dna_hash())
            .map_err(|_| DnaError::DnaMissing(self.dna_hash().to_owned()))?)
    }

    /// Accessor for the p2p_agents_db backing this Cell
    pub(crate) fn p2p_agents_db(&self) -> &DbWrite<DbKindP2pAgents> {
        &self.space.p2p_agents_db
    }

    /// Accessor for the authored database backing this Cell
    pub(crate) fn get_or_create_authored_db(&self) -> CellResult<DbWrite<DbKindAuthored>> {
        Ok(self
            .space
            .get_or_create_authored_db(self.id.agent_pubkey().clone())?)
    }

    /// Accessor for the authored database backing this Cell
    pub(crate) fn dht_db(&self) -> &DbWrite<DbKindDht> {
        &self.space.dht_db
    }

    pub(crate) fn cache(&self) -> &DbWrite<DbKindCache> {
        &self.space.cache_db
    }

    pub(crate) fn notify_authored_ops_moved_to_limbo(&self) {
        self.queue_triggers
            .integrate_dht_ops
            .trigger(&"notify_authored_ops_moved_to_limbo");
    }

    pub(crate) fn publish_authored_ops(&self) {
        self.queue_triggers
            .publish_dht_ops
            .trigger(&"publish_authored_ops");
    }

    pub(crate) fn countersigning_trigger(&self) -> TriggerSender {
        self.queue_triggers.countersigning.clone()
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
