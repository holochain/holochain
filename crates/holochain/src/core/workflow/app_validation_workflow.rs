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
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::op::EntryCreationHeader;
use holochain_zome_types::op::Op;
use rusqlite::Transaction;
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

#[instrument(skip(workspace, trigger_integration, conductor_handle, network))]
pub async fn app_validation_workflow(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    trigger_integration: TriggerSender,
    conductor_handle: ConductorHandle,
    network: HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let complete =
        app_validation_workflow_inner(dna_hash, workspace, conductor_handle, &network).await?;
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
) -> WorkflowResult<WorkComplete> {
    let env = workspace.dht_env.clone().into();
    let sorted_ops = validation_query::get_ops_to_app_validate(&env).await?;
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
                let dependency = get_dependency(op.get_type(), &op.header());
                let op_light = op.to_light();

                // Validate this op
                let mut cascade = workspace.full_cascade(network.clone());
                let r = match dhtop_to_op(op, &mut cascade).await {
                    Ok(op) => {
                        validate_op_outer(dna_hash, &op, &conductor_handle, &(*workspace), &network)
                            .await
                    }
                    Err(e) => Err(e),
                };
                (op_hash, dependency, op_light, r)
            }
        }
    });

    // Create a stream of concurrent validation futures.
    // This will run NUM_CONCURRENT_OPS validation futures concurrently and
    // return up to NUM_CONCURRENT_OPS * 100 results.
    use futures::stream::StreamExt;
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
        let (t, a, r) = workspace
            .dht_env
            .async_commit(move |mut txn| {
                let mut total = 0;
                let mut awaiting = 0;
                let mut rejected = 0;
                for outcome in chunk.into_iter().flatten() {
                    let (op_hash, dependency, op_light, outcome) = outcome;
                    // Get the outcome or return the error
                    let outcome = outcome.or_else(|outcome_or_err| outcome_or_err.try_into())?;

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
                                put_integrated(&mut txn, op_hash, ValidationStatus::Valid)?;
                            } else {
                                put_integration_limbo(&mut txn, op_hash, ValidationStatus::Valid)?;
                            }
                        }
                        Outcome::AwaitingDeps(deps) => {
                            awaiting += 1;
                            let status = ValidationLimboStatus::AwaitingAppDeps(deps);
                            put_validation_limbo(&mut txn, op_hash, status)?;
                        }
                        Outcome::Rejected(_) => {
                            rejected += 1;
                            tracing::warn!("Received invalid op! Warrants aren't implemented yet, so we can't do anything about this right now, but be warned that somebody on the network has maliciously hacked their node.\nOp: {:?}", op_light);
                            if let Dependency::Null = dependency {
                                put_integrated(&mut txn, op_hash, ValidationStatus::Rejected)?;
                            } else {
                                put_integration_limbo(&mut txn, op_hash, ValidationStatus::Rejected)?;
                            }
                        }
                    }
                }
                WorkflowResult::Ok((total, awaiting, rejected))
            })
            .await?;
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

pub fn to_single_zome(zomes_to_invoke: ZomesToInvoke) -> AppValidationResult<Zome> {
    match zomes_to_invoke {
        ZomesToInvoke::All => Err(AppValidationError::LinkMultipleZomes),
        ZomesToInvoke::One(z) => Ok(z),
    }
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
    let (header, _) = shh.into_inner();
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
        Op::StoreEntry { header, entry } => Element::new(header.into_shh(), Some(entry)),
        Op::RegisterUpdate {
            update, new_entry, ..
        } => Element::new(update.into_shh(), Some(new_entry)),
        Op::RegisterDelete { delete, .. } => Element::new(delete.into_shh(), None),
        Op::RegisterAgentActivity { header } => Element::new(header.into_shh(), activity_entry),
        Op::RegisterCreateLink { create_link, .. } => Element::new(create_link.into_shh(), None),
        Op::RegisterDeleteLink { delete_link, .. } => Element::new(delete_link.into_shh(), None),
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
                .and_then(|e| {
                    let sh = e.into_inner().0;
                    NewEntryHeader::try_from(sh.0).ok().map(|h| h.into())
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
                .and_then(|e| {
                    let sh = e.into_inner().0;
                    NewEntryHeader::try_from(sh.0).ok().map(|h| h.into())
                })
                .ok_or_else(|| Outcome::awaiting(&delete.deletes_address))?;
            Op::RegisterDelete {
                delete: SignedHashed::new(delete, signature),
                original_header,
                original_entry,
            }
        }
        DhtOp::RegisterAddLink(signature, create_link) => {
            let base = cascade
                .retrieve_entry(create_link.base_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&create_link.base_address))?;

            let target = cascade
                .retrieve_entry(create_link.target_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&create_link.target_address))?;
            Op::RegisterCreateLink {
                create_link: SignedHashed::new(create_link, signature),
                base,
                target,
            }
        }
        DhtOp::RegisterRemoveLink(signature, delete_link) => {
            let create_link = cascade
                .retrieve_header(delete_link.link_add_address.clone(), Default::default())
                .await?
                .and_then(|e| {
                    let sh = e.into_inner().0;
                    CreateLink::try_from(sh.0).ok().map(|h| h.into())
                })
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
    let workspace = workspace.validation_workspace().await?;

    // Get the dna file
    let dna_file = conductor_handle
        .get_dna(dna_hash.as_ref())
        .ok_or_else(|| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    // Create the ribosome
    let ribosome = RealRibosome::new(dna_file);
    validate_op(op, workspace, network, &ribosome).await
}

pub async fn validate_op(
    op: &Op,
    workspace: HostFnWorkspaceRead,
    network: &HolochainP2pDna,
    ribosome: &impl RibosomeT,
) -> AppValidationOutcome<Outcome> {
    let zomes_to_invoke = match op {
        Op::RegisterAgentActivity { .. } | Op::StoreElement { .. } => ZomesToInvoke::All,
        Op::StoreEntry {
            header: SignedHashed { header, .. },
            ..
        } => entry_creation_zomes_to_invoke(header, ribosome.dna_def())?,
        Op::RegisterUpdate {
            original_header, ..
        }
        | Op::RegisterDelete {
            original_header, ..
        } => entry_creation_zomes_to_invoke(original_header, ribosome.dna_def())?,
        Op::RegisterCreateLink {
            create_link: SignedHashed { header, .. },
            ..
        } => create_link_zomes_to_invoke(header, ribosome.dna_def())?,
        Op::RegisterDeleteLink {
            create_link: header,
            ..
        } => create_link_zomes_to_invoke(header, ribosome.dna_def())?,
    };

    let invocation = ValidateInvocation::new(zomes_to_invoke, op)
        .map_err(|e| AppValidationError::RibosomeError(e.into()))?;
    let outcome = run_validation_callback_inner(invocation, ribosome, workspace, network.clone())?;

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
        _ => Ok(ZomesToInvoke::All),
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
        .zomes
        .get(zome_index)
        .ok_or(AppValidationError::ZomeId(zome_id))?
        .clone()
        .into())
}

fn run_validation_callback_inner(
    invocation: ValidateInvocation,
    ribosome: &impl RibosomeT,
    workspace_lock: HostFnWorkspaceRead,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let validate: ValidateResult =
        ribosome.run_validate(ValidateHostAccess::new(workspace_lock, network), invocation)?;
    match validate {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(hashes) => Ok(Outcome::AwaitingDeps(hashes)),
    }
}

pub struct AppValidationWorkspace {
    authored_env: DbRead<DbKindAuthored>,
    dht_env: DbWrite<DbKindDht>,
    cache: DbWrite<DbKindCache>,
    keystore: MetaLairClient,
}

impl AppValidationWorkspace {
    pub fn new(
        authored_env: DbRead<DbKindAuthored>,
        dht_env: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
    ) -> Self {
        Self {
            authored_env,
            dht_env,
            cache,
            keystore,
        }
    }

    pub async fn validation_workspace(&self) -> AppValidationResult<HostFnWorkspaceRead> {
        Ok(HostFnWorkspace::new(
            self.authored_env.clone(),
            self.dht_env.clone().into(),
            self.cache.clone(),
            self.keystore.clone(),
            None,
        )
        .await?)
    }

    pub fn full_cascade<Network: HolochainP2pDnaT + Clone + 'static + Send>(
        &self,
        network: Network,
    ) -> Cascade<Network> {
        Cascade::empty()
            .with_authored(self.authored_env.clone())
            .with_dht(self.dht_env.clone().into())
            .with_network(network, self.cache.clone())
    }
}

pub fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: DhtOpHash,
    status: ValidationLimboStatus,
) -> WorkflowResult<()> {
    set_validation_stage(txn, hash, status)?;
    Ok(())
}

pub fn put_integration_limbo(
    txn: &mut Transaction<'_>,
    hash: DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash.clone(), status)?;
    set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    Ok(())
}

pub fn put_integrated(
    txn: &mut Transaction<'_>,
    hash: DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash.clone(), status)?;
    // This set the validation stage to pending which is correct when
    // it's integrated.
    set_validation_stage(txn, hash.clone(), ValidationLimboStatus::Pending)?;
    set_when_integrated(txn, hash, Timestamp::now())?;
    Ok(())
}
