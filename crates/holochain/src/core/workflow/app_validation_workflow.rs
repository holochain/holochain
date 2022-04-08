//! The workflow and queue consumer for sys validation

use std::convert::TryInto;
use std::sync::Arc;

use super::error::WorkflowResult;
use super::sys_validation_workflow::validation_query;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomesToInvoke;
use error::AppValidationResult;
pub use error::*;
use futures::stream::StreamExt;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::*;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_zome_types::op::EntryCreationHeader;
use holochain_zome_types::op::Op;
use rusqlite::Transaction;
use std::collections::HashSet;
use tracing::*;
pub use types::Outcome;

#[cfg(todo_redo_old_tests)]
mod network_call_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod validation_tests;

mod error;
mod types;
pub mod validation_package;

const NUM_CONCURRENT_OPS: usize = 50;

#[instrument(skip(
    workspace,
    trigger_integration,
    conductor_handle,
    network,
    dht_query_cache
))]
pub async fn app_validation_workflow(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    trigger_integration: TriggerSender,
    conductor_handle: ConductorHandle,
    network: HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
) -> WorkflowResult<WorkComplete> {
    let complete = app_validation_workflow_inner(
        dna_hash,
        workspace,
        conductor_handle,
        &network,
        dht_query_cache,
    )
    .await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // trigger other workflows
    trigger_integration.trigger();

    Ok(complete)
}

async fn app_validation_workflow_inner(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    conductor_handle: ConductorHandle,
    network: &HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
) -> WorkflowResult<WorkComplete> {
    let db = workspace.dht_db.clone().into();
    let sorted_ops = validation_query::get_ops_to_app_validate(&db).await?;
    let start_len = sorted_ops.len();
    tracing::debug!("validating {} ops", start_len);
    let start = (start_len >= NUM_CONCURRENT_OPS).then(std::time::Instant::now);
    let saturated = start.is_some();

    // Validate all the ops
    let iter = sorted_ops.into_iter().map({
        let network = network.clone();
        let workspace = workspace.clone();
        move |so| {
            let network = network.clone();
            let conductor_handle = conductor_handle.clone();
            let workspace = workspace.clone();
            let dna_hash = dna_hash.clone();
            async move {
                let (op, op_hash) = so.into_inner();
                let op_type = op.get_type();
                let header = op.header();
                let dependency = get_dependency(op_type, &header);
                let op_light = op.to_light();

                // If this is agent activity, track it for the cache.
                let activity = matches!(op_type, DhtOpType::RegisterAgentActivity).then(|| {
                    (
                        header.author().clone(),
                        header.header_seq(),
                        matches!(dependency, Dependency::Null),
                    )
                });

                // Validate this op
                let mut cascade = workspace.full_cascade(network.clone());
                let r = match dhtop_to_op(op, &mut cascade).await {
                    Ok(op) => {
                        validate_op_outer(dna_hash, &op, &conductor_handle, &(*workspace), &network)
                            .await
                    }
                    Err(e) => Err(e),
                };
                (op_hash, dependency, op_light, r, activity)
            }
        }
    });

    // Create a stream of concurrent validation futures.
    // This will run NUM_CONCURRENT_OPS validation futures concurrently and
    // return up to NUM_CONCURRENT_OPS * 100 results.
    let mut iter = futures::stream::iter(iter)
        .buffer_unordered(NUM_CONCURRENT_OPS)
        .ready_chunks(NUM_CONCURRENT_OPS * 100);

    // Spawn a task to actually drive the stream.
    // This allows the stream to make progress in the background while
    // we are committing previous results to the database.
    let (tx, rx) = tokio::sync::mpsc::channel(NUM_CONCURRENT_OPS * 100);
    let jh = tokio::spawn(async move {
        while let Some(op) = iter.next().await {
            // Send the result to task that will commit to the database.
            if tx.send(op).await.is_err() {
                tracing::warn!("app validation task has failed to send ops. This is not a problem if the conductor is shutting down");
                break;
            }
        }
    });

    // Create a stream that will chunk up to NUM_CONCURRENT_OPS * 100 ready results.
    let mut iter =
        tokio_stream::wrappers::ReceiverStream::new(rx).ready_chunks(NUM_CONCURRENT_OPS * 100);

    let mut total = 0;
    let mut round_time = start.is_some().then(std::time::Instant::now);
    // Pull in a chunk of results.
    while let Some(chunk) = iter.next().await {
        tracing::debug!(
            "Committing {} ops",
            chunk.iter().map(|c| c.len()).sum::<usize>()
        );
        let (t, a, r, activity) = workspace
            .dht_db
            .async_commit(move |txn| {
                let mut total = 0;
                let mut awaiting = 0;
                let mut rejected = 0;
                let mut agent_activity = Vec::new();
                for outcome in chunk.into_iter().flatten() {
                    let (op_hash, dependency, op_light, outcome, activity) = outcome;
                    // Get the outcome or return the error
                    let outcome = outcome.or_else(|outcome_or_err| outcome_or_err.try_into())?;

                    // Collect all agent activity.
                    if let Some(activity) = activity {
                        // If the activity is accepted or rejected then it's ready to integrate.
                        if matches!(&outcome, Outcome::Accepted | Outcome::Rejected(_)) {
                            agent_activity.push(activity);
                        }
                    }


                    if let Outcome::AwaitingDeps(_) | Outcome::Rejected(_) = &outcome {
                        warn!(
                            msg = "DhtOp has failed app validation",
                            outcome = ?outcome,
                        );
                    }
                    match outcome {
                        Outcome::Accepted => {
                            total += 1;
                            if let Dependency::Null = dependency {
                                put_integrated(txn, &op_hash, ValidationStatus::Valid)?;
                            } else {
                                put_integration_limbo(txn, &op_hash, ValidationStatus::Valid)?;
                            }
                        }
                        Outcome::AwaitingDeps(deps) => {
                            awaiting += 1;
                            let status = ValidationLimboStatus::AwaitingAppDeps(deps);
                            put_validation_limbo(txn, &op_hash, status)?;
                        }
                        Outcome::Rejected(_) => {
                            rejected += 1;
                            tracing::warn!("Received invalid op! Warrants aren't implemented yet, so we can't do anything about this right now, but be warned that somebody on the network has maliciously hacked their node.\nOp: {:?}", op_light);
                            if let Dependency::Null = dependency {
                                put_integrated(txn, &op_hash, ValidationStatus::Rejected)?;
                            } else {
                                put_integration_limbo(txn, &op_hash, ValidationStatus::Rejected)?;
                            }
                        }
                    }
                }
                WorkflowResult::Ok((total, awaiting, rejected, agent_activity))
            })
            .await?;
        // Once the database transaction is committed, add agent activity to the cache
        // that is ready for integration.
        for (author, seq, has_no_dependency) in activity {
            // Any activity with no dependency is integrated in this workflow.
            // TODO: This will no longer be true when [#1212](https://github.com/holochain/holochain/pull/1212) lands.
            if has_no_dependency {
                dht_query_cache
                    .set_activity_to_integrated(&author, seq)
                    .await?;
            } else {
                dht_query_cache
                    .set_activity_ready_to_integrate(&author, seq)
                    .await?;
            }
        }
        total += t;
        if let (Some(start), Some(round_time)) = (start, &mut round_time) {
            let round_el = round_time.elapsed();
            *round_time = std::time::Instant::now();
            let avg_ops_ps = total as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
            let ops_ps = t as f64 / round_el.as_micros() as f64 * 1_000_000.0;
            tracing::info!(
                "App validation is saturated. Util {:.2}%. OPS/s avg {:.2}, this round {:.2}",
                (start_len - total) as f64 / NUM_CONCURRENT_OPS as f64 * 100.0,
                avg_ops_ps,
                ops_ps
            );
        }
        tracing::debug!(
            "{} committed, {} awaiting sys dep, {} rejected. {} committed this round",
            t,
            a,
            r,
            total
        );
    }
    jh.await?;
    tracing::debug!("accepted {} ops", total);
    Ok(if saturated {
        WorkComplete::Incomplete
    } else {
        WorkComplete::Complete
    })
}

pub async fn element_to_op(
    element: Element,
    op_type: DhtOpType,
    cascade: &mut Cascade,
) -> AppValidationOutcome<(Op, Option<Entry>)> {
    use DhtOpType::*;
    let mut activity_entry = None;
    let (shh, entry) = element.into_inner();
    let mut entry = entry.into_option();
    let header = shh.into();
    // Register agent activity doesn't store the entry so we need to
    // save it so we can reconstruct the element later.
    if matches!(op_type, RegisterAgentActivity) {
        activity_entry = entry.take();
    }
    let dht_op = DhtOp::from_type(op_type, header, entry)?;
    Ok((dhtop_to_op(dht_op, cascade).await?, activity_entry))
}

pub fn op_to_element(op: Op, activity_entry: Option<Entry>) -> Element {
    match op {
        Op::StoreElement { element } => element,
        Op::StoreEntry { header, entry } => {
            Element::new(SignedHeaderHashed::raw_from_same_hash(header), Some(entry))
        }
        Op::RegisterUpdate {
            update, new_entry, ..
        } => Element::new(
            SignedHeaderHashed::raw_from_same_hash(update),
            Some(new_entry),
        ),
        Op::RegisterDelete { delete, .. } => {
            Element::new(SignedHeaderHashed::raw_from_same_hash(delete), None)
        }
        Op::RegisterAgentActivity { header } => Element::new(
            SignedHeaderHashed::raw_from_same_hash(header),
            activity_entry,
        ),
        Op::RegisterCreateLink { create_link, .. } => {
            Element::new(SignedHeaderHashed::raw_from_same_hash(create_link), None)
        }
        Op::RegisterDeleteLink { delete_link, .. } => {
            Element::new(SignedHeaderHashed::raw_from_same_hash(delete_link), None)
        }
    }
}

async fn dhtop_to_op(op: DhtOp, cascade: &mut Cascade) -> AppValidationOutcome<Op> {
    let op = match op {
        DhtOp::StoreElement(signature, header, entry) => Op::StoreElement {
            element: Element::new(
                SignedHeaderHashed::with_presigned(
                    HeaderHashed::from_content_sync(header),
                    signature,
                ),
                entry.map(|e| *e),
            ),
        },
        DhtOp::StoreEntry(signature, header, entry) => Op::StoreEntry {
            header: SignedHashed::new(header.into(), signature),
            entry: *entry,
        },
        DhtOp::RegisterAgentActivity(signature, header) => Op::RegisterAgentActivity {
            header: SignedHeaderHashed::with_presigned(
                HeaderHashed::from_content_sync(header),
                signature,
            ),
        },
        DhtOp::RegisterUpdatedContent(signature, update, entry)
        | DhtOp::RegisterUpdatedElement(signature, update, entry) => {
            let new_entry = match entry {
                Some(entry) => *entry,
                None => cascade
                    .retrieve_entry(update.entry_hash.clone(), Default::default())
                    .await?
                    .map(|e| e.into_content())
                    .ok_or_else(|| Outcome::awaiting(&update.entry_hash))?,
            };

            let original_entry = cascade
                .retrieve_entry(update.original_entry_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&update.original_entry_address))?;

            let original_header = cascade
                .retrieve_header(update.original_header_address.clone(), Default::default())
                .await?
                .and_then(|sh| {
                    NewEntryHeader::try_from(sh.hashed.content)
                        .ok()
                        .map(|h| h.into())
                })
                .ok_or_else(|| Outcome::awaiting(&update.original_header_address))?;
            Op::RegisterUpdate {
                update: SignedHashed::new(update, signature),
                new_entry,
                original_header,
                original_entry,
            }
        }
        DhtOp::RegisterDeletedBy(signature, delete)
        | DhtOp::RegisterDeletedEntryHeader(signature, delete) => {
            let original_entry = cascade
                .retrieve_entry(delete.deletes_entry_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&delete.deletes_entry_address))?;

            let original_header = cascade
                .retrieve_header(delete.deletes_address.clone(), Default::default())
                .await?
                .and_then(|sh| {
                    NewEntryHeader::try_from(sh.hashed.content)
                        .ok()
                        .map(|h| h.into())
                })
                .ok_or_else(|| Outcome::awaiting(&delete.deletes_address))?;
            Op::RegisterDelete {
                delete: SignedHashed::new(delete, signature),
                original_header,
                original_entry,
            }
        }
        DhtOp::RegisterAddLink(signature, create_link) => Op::RegisterCreateLink {
            create_link: SignedHashed::new(create_link, signature),
        },
        DhtOp::RegisterRemoveLink(signature, delete_link) => {
            let create_link = cascade
                .retrieve_header(delete_link.link_add_address.clone(), Default::default())
                .await?
                .and_then(|sh| CreateLink::try_from(sh.hashed.content).ok())
                .ok_or_else(|| Outcome::awaiting(&delete_link.link_add_address))?;
            Op::RegisterDeleteLink {
                delete_link: SignedHashed::new(delete_link, signature),
                create_link,
            }
        }
    };
    Ok(op)
}

async fn validate_op_outer(
    dna_hash: Arc<DnaHash>,
    op: &Op,
    conductor_handle: &ConductorHandle,
    workspace: &AppValidationWorkspace,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Outcome> {
    // Get the workspace for the validation calls
    let host_fn_workspace = workspace.validation_workspace().await?;

    // Get the dna file
    let dna_file = conductor_handle
        .get_dna_file(dna_hash.as_ref())
        .ok_or_else(|| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    // Create the ribosome
    let ribosome = RealRibosome::new(dna_file);
    validate_op(op, host_fn_workspace, network, &ribosome).await
}

pub async fn validate_op<R>(
    op: &Op,
    workspace: HostFnWorkspaceRead,
    network: &HolochainP2pDna,
    ribosome: &R,
) -> AppValidationOutcome<Outcome>
where
    R: RibosomeT,
{
    let zomes_to_invoke = match op {
        Op::RegisterAgentActivity { .. } | Op::StoreElement { .. } => ZomesToInvoke::AllIntegrity,
        Op::StoreEntry {
            header:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: header, ..
                        },
                    ..
                },
            ..
        } => entry_creation_zomes_to_invoke(header, ribosome.dna_def())?,
        Op::RegisterUpdate {
            original_header, ..
        }
        | Op::RegisterDelete {
            original_header, ..
        } => entry_creation_zomes_to_invoke(original_header, ribosome.dna_def())?,
        Op::RegisterCreateLink {
            create_link:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: header, ..
                        },
                    ..
                },
            ..
        } => create_link_zomes_to_invoke(header, ribosome.dna_def())?,
        Op::RegisterDeleteLink {
            create_link: header,
            ..
        } => create_link_zomes_to_invoke(header, ribosome.dna_def())?,
    };

    let invocation = ValidateInvocation::new(zomes_to_invoke, op)
        .map_err(|e| AppValidationError::RibosomeError(e.into()))?;
    let outcome = run_validation_callback_inner(
        invocation,
        ribosome,
        workspace,
        network.clone(),
        (HashSet::<AnyDhtHash>::new(), 0),
    )
    .await?;

    Ok(outcome)
}

pub fn entry_creation_zomes_to_invoke(
    header: &EntryCreationHeader,
    dna_def: &DnaDef,
) -> AppValidationOutcome<ZomesToInvoke> {
    match header {
        EntryCreationHeader::Create(Create {
            entry_type: EntryType::App(aet),
            ..
        })
        | EntryCreationHeader::Update(Update {
            entry_type: EntryType::App(aet),
            ..
        }) => {
            let zome = zome_id_to_zome(aet.zome_id(), dna_def)?;
            Ok(ZomesToInvoke::One(zome))
        }
        _ => Ok(ZomesToInvoke::AllIntegrity),
    }
}

fn create_link_zomes_to_invoke(
    create_link: &CreateLink,
    dna_def: &DnaDef,
) -> AppValidationOutcome<ZomesToInvoke> {
    let zome = zome_id_to_zome(create_link.zome_id, dna_def)?;
    Ok(ZomesToInvoke::One(zome))
}

fn zome_id_to_zome(zome_id: ZomeId, dna_def: &DnaDef) -> AppValidationResult<Zome> {
    let zome_index = u8::from(zome_id) as usize;
    Ok(dna_def
        .integrity_zomes
        .get(zome_index)
        .ok_or(AppValidationError::ZomeId(zome_id))?
        .clone()
        .into())
}

#[async_recursion::async_recursion]
async fn run_validation_callback_inner<R>(
    invocation: ValidateInvocation,
    ribosome: &R,
    workspace_read: HostFnWorkspaceRead,
    network: HolochainP2pDna,
    (mut fetched_deps, recursion_depth): (HashSet<AnyDhtHash>, usize),
) -> AppValidationResult<Outcome>
where
    R: RibosomeT,
{
    let validate_result = ribosome.run_validate(
        ValidateHostAccess::new(workspace_read.clone(), network.clone()),
        invocation.clone(),
    )?;
    match validate_result {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(hashes) => {
            // This is the base case where we've been recursing and start seeing
            // all the same hashes unresolved that we already tried to fetch.
            // At this point we should just give up on the inline recursing and
            // let some future background task attempt to fetch these hashes
            // again. Hopefully by then the hashes are fetchable.
            // 20 is a completely arbitrary max recursion depth.
            if recursion_depth < 20 || hashes.iter().all(|hash| fetched_deps.contains(hash)) {
                Ok(Outcome::AwaitingDeps(hashes))
            } else {
                let in_flight = hashes.into_iter().map(|hash| async {
                    let cascade_workspace = workspace_read.clone();
                    let mut cascade =
                        Cascade::from_workspace_network(&cascade_workspace, network.clone());
                    cascade
                        .fetch_element(hash.clone(), NetworkGetOptions::must_get_options())
                        .await?;
                    Ok(hash)
                });
                let results: Vec<_> = futures::stream::iter(in_flight)
                    // 10 is completely arbitrary.
                    .buffered(10)
                    .collect()
                    .await;
                let results: AppValidationResult<Vec<_>> = results.into_iter().collect();
                for hash in results? {
                    fetched_deps.insert(hash);
                }
                run_validation_callback_inner(
                    invocation,
                    ribosome,
                    workspace_read,
                    network,
                    (fetched_deps, recursion_depth + 1),
                )
                .await
            }
        }
    }
}

pub struct AppValidationWorkspace {
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_db_cache: DhtDbQueryCache,
    cache: DbWrite<DbKindCache>,
    keystore: MetaLairClient,
    dna_def: Arc<DnaDef>,
}

impl AppValidationWorkspace {
    pub fn new(
        authored_db: DbRead<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        dna_def: Arc<DnaDef>,
    ) -> Self {
        Self {
            authored_db,
            dht_db,
            dht_db_cache,
            cache,
            keystore,
            dna_def,
        }
    }

    pub async fn validation_workspace(&self) -> AppValidationResult<HostFnWorkspaceRead> {
        Ok(HostFnWorkspace::new(
            self.authored_db.clone(),
            self.dht_db.clone().into(),
            self.dht_db_cache.clone(),
            self.cache.clone(),
            self.keystore.clone(),
            None,
            self.dna_def.clone(),
        )
        .await?)
    }

    pub fn full_cascade<Network: HolochainP2pDnaT + Clone + 'static + Send>(
        &self,
        network: Network,
    ) -> Cascade<Network> {
        Cascade::empty()
            .with_authored(self.authored_db.clone())
            .with_dht(self.dht_db.clone().into())
            .with_network(network, self.cache.clone())
    }
}

pub fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationLimboStatus,
) -> WorkflowResult<()> {
    set_validation_stage(txn, hash, status)?;
    Ok(())
}

pub fn put_integration_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash, status)?;
    set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    Ok(())
}

pub fn put_integrated(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash, status)?;
    // This set the validation stage to pending which is correct when
    // it's integrated.
    set_validation_stage(txn, hash, ValidationLimboStatus::Pending)?;
    set_when_integrated(txn, hash, Timestamp::now())?;
    Ok(())
}
