//! The workflow and queue consumer for sys validation

use super::error::WorkflowResult;
use super::sys_validation_workflow::validation_query;
use crate::conductor::entry_def_store::get_entry_def;
use crate::conductor::Conductor;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::SysValidationError;
use crate::core::SysValidationResult;
use crate::core::ValidationOutcome;
pub use error::*;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::*;
use rusqlite::Transaction;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;
pub use types::Outcome;

#[cfg(todo_redo_old_tests)]
mod network_call_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod validation_tests;

#[cfg(test)]
mod unit_tests;

mod error;
mod types;

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
    trigger_integration.trigger(&"app_validation_workflow");

    Ok(complete)
}

async fn app_validation_workflow_inner(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    conductor: ConductorHandle,
    network: &HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
) -> WorkflowResult<WorkComplete> {
    let db = workspace.dht_db.clone().into();
    let sorted_ops = validation_query::get_ops_to_app_validate(&db).await?;
    let num_ops_to_validate = sorted_ops.len();
    tracing::debug!("validating {num_ops_to_validate} ops");
    let sleuth_id = conductor.config.sleuth_id();

    // Build an iterator of all op validations
    let iter = sorted_ops.into_iter().map({
        let network = network.clone();
        let workspace = workspace.clone();
        move |so| {
            let network = network.clone();
            let conductor = conductor.clone();
            let workspace = workspace.clone();
            let dna_hash = dna_hash.clone();
            async move {
                let (op, op_hash) = so.into_inner();
                let op_type = op.get_type();
                let action = op.action();
                let dependency = op.sys_validation_dependency();
                let op_lite = op.to_lite();

                // If this is agent activity, track it for the cache.
                let activity = matches!(op_type, DhtOpType::RegisterAgentActivity).then(|| {
                    (
                        action.author().clone(),
                        action.action_seq(),
                        dependency.is_none(),
                    )
                });

                // Validate this op
                let cascade = Arc::new(workspace.full_cascade(network.clone()));
                let r = match dhtop_to_op(op, cascade).await {
                    Ok(op) => {
                        validate_op_outer(dna_hash, &op, &conductor, &workspace, &network).await
                    }
                    Err(e) => Err(e),
                };
                (op_hash, dependency, op_lite, r, activity)
            }
        }
    });

    let validation_results = futures::future::join_all(iter).await;

    tracing::debug!("Committing {} ops", validation_results.len());
    let mut ops_validated = 0;
    let sleuth_id = sleuth_id.clone();
    let (accepted_ops, awaiting_ops, rejected_ops, activity) = workspace
        .dht_db
        .write_async(move |txn| {
            let mut accepted = 0;
            let mut awaiting = 0;
            let mut rejected = 0;
            let mut agent_activity = Vec::new();
            for outcome in validation_results {
                let (op_hash, dependency, op_lite, outcome, activity) = outcome;
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
                        accepted += 1;
                        aitia::trace!(&hc_sleuth::Event::AppValidated {
                            by: sleuth_id.clone(),
                            op: op_hash.clone()
                        });

                        if dependency.is_none() {
                            aitia::trace!(&hc_sleuth::Event::Integrated {
                                by: sleuth_id.clone(),
                                op: op_hash.clone()
                            });

                            put_integrated(txn, &op_hash, ValidationStatus::Valid)?;
                        } else {
                            put_integration_limbo(txn, &op_hash, ValidationStatus::Valid)?;
                        }
                    }
                    Outcome::AwaitingDeps(deps) => {
                        awaiting += 1;
                        let status = ValidationStage::AwaitingAppDeps(deps);
                        put_validation_limbo(txn, &op_hash, status)?;
                    }
                    Outcome::Rejected(_) => {
                        rejected += 1;
                        tracing::info!(
                            "Received invalid op. The op author will be blocked.\nOp: {:?}",
                            op_lite
                        );
                        if dependency.is_none() {
                            put_integrated(txn, &op_hash, ValidationStatus::Rejected)?;
                        } else {
                            put_integration_limbo(txn, &op_hash, ValidationStatus::Rejected)?;
                        }
                    }
                }
            }
            WorkflowResult::Ok((accepted, awaiting, rejected, agent_activity))
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
    ops_validated += accepted_ops;
    ops_validated += rejected_ops;
    tracing::debug!("{ops_validated} out of {num_ops_to_validate} validated: {accepted_ops} accepted, {awaiting_ops} awaiting deps, {rejected_ops} rejected.");

    Ok(if ops_validated < num_ops_to_validate {
        // trigger app validation workflow again in 10 seconds
        WorkComplete::Incomplete(Some(Duration::from_secs(10)))
    } else {
        WorkComplete::Complete
    })
}

pub async fn record_to_op(
    record: Record,
    op_type: DhtOpType,
    cascade: Arc<impl Cascade>,
) -> AppValidationOutcome<(Op, Option<Entry>)> {
    use DhtOpType::*;

    // Hide private data where appropriate
    let (record, mut hidden_entry) = if matches!(op_type, DhtOpType::StoreEntry) {
        // We don't want to hide private data for a StoreEntry, because when doing
        // inline validation as an author, we want to validate and integrate our own entry!
        // Publishing and gossip rules state that a private StoreEntry will never be transmitted
        // to another node.
        (record, None)
    } else {
        // All other records have private entry data hidden, including from ourselves if we are
        // authoring private data.
        record.privatized()
    };

    let (shh, entry) = record.into_inner();
    let mut entry = entry.into_option();
    let action = shh.into();
    // Register agent activity doesn't store the entry so we need to
    // save it so we can reconstruct the record later.
    if matches!(op_type, RegisterAgentActivity) {
        hidden_entry = entry.take().or(hidden_entry);
    }
    let dht_op = DhtOp::from_type(op_type, action, entry)?;
    Ok((dhtop_to_op(dht_op, cascade).await?, hidden_entry))
}

async fn dhtop_to_op(op: DhtOp, cascade: Arc<impl Cascade>) -> AppValidationOutcome<Op> {
    let op = match op {
        DhtOp::StoreRecord(signature, action, entry) => Op::StoreRecord(StoreRecord {
            record: Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                entry.into_option(),
            ),
        }),
        DhtOp::StoreEntry(signature, action, entry) => Op::StoreEntry(StoreEntry {
            action: SignedHashed::new_unchecked(action.into(), signature),
            entry,
        }),
        DhtOp::RegisterAgentActivity(signature, action) => {
            Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                cached_entry: None,
            })
        }
        DhtOp::RegisterUpdatedContent(signature, update, entry)
        | DhtOp::RegisterUpdatedRecord(signature, update, entry) => {
            let new_entry = match update.entry_type.visibility() {
                EntryVisibility::Public => match entry.into_option() {
                    Some(entry) => Some(entry),
                    None => Some(
                        cascade
                            .retrieve_entry(update.entry_hash.clone(), Default::default())
                            .await?
                            .map(|(e, _)| e.into_content())
                            .ok_or_else(|| Outcome::awaiting(&update.entry_hash))?,
                    ),
                },
                _ => None,
            };
            let original_entry = if let EntryVisibility::Public = update.entry_type.visibility() {
                Some(
                    cascade
                        .retrieve_entry(update.original_entry_address.clone(), Default::default())
                        .await?
                        .map(|(e, _)| e.into_content())
                        .ok_or_else(|| Outcome::awaiting(&update.original_entry_address))?,
                )
            } else {
                None
            };

            let original_action = cascade
                .retrieve_action(update.original_action_address.clone(), Default::default())
                .await?
                .and_then(|(sh, _)| {
                    NewEntryAction::try_from(sh.hashed.content)
                        .ok()
                        .map(|h| h.into())
                })
                .ok_or_else(|| Outcome::awaiting(&update.original_action_address))?;
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed::new_unchecked(update, signature),
                new_entry,
                original_action,
                original_entry,
            })
        }
        DhtOp::RegisterDeletedBy(signature, delete)
        | DhtOp::RegisterDeletedEntryAction(signature, delete) => {
            let original_action: EntryCreationAction = cascade
                .retrieve_action(delete.deletes_address.clone(), Default::default())
                .await?
                .and_then(|(sh, _)| {
                    NewEntryAction::try_from(sh.hashed.content)
                        .ok()
                        .map(|h| h.into())
                })
                .ok_or_else(|| Outcome::awaiting(&delete.deletes_address))?;

            let original_entry = if let EntryVisibility::Public =
                original_action.entry_type().visibility()
            {
                Some(
                    cascade
                        .retrieve_entry(delete.deletes_entry_address.clone(), Default::default())
                        .await?
                        .map(|(e, _)| e.into_content())
                        .ok_or_else(|| Outcome::awaiting(&delete.deletes_entry_address))?,
                )
            } else {
                None
            };
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed::new_unchecked(delete, signature),
                original_action,
                original_entry,
            })
        }
        DhtOp::RegisterAddLink(signature, create_link) => {
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed::new_unchecked(create_link, signature),
            })
        }
        DhtOp::RegisterRemoveLink(signature, delete_link) => {
            let create_link = cascade
                .retrieve_action(delete_link.link_add_address.clone(), Default::default())
                .await?
                .and_then(|(sh, _)| CreateLink::try_from(sh.hashed.content).ok())
                .ok_or_else(|| Outcome::awaiting(&delete_link.link_add_address))?;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: SignedHashed::new_unchecked(delete_link, signature),
                create_link,
            })
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

    // Get the ribosome
    let ribosome = conductor_handle
        .get_ribosome(dna_hash.as_ref())
        .map_err(|_| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    validate_op(op, host_fn_workspace, network, &ribosome, conductor_handle).await
}

pub async fn validate_op<R>(
    op: &Op,
    workspace: HostFnWorkspaceRead,
    network: &HolochainP2pDna,
    ribosome: &R,
    conductor_handle: &ConductorHandle,
) -> AppValidationOutcome<Outcome>
where
    R: RibosomeT,
{
    check_entry_def(op, &network.dna_hash(), conductor_handle)
        .await
        .map_err(AppValidationError::SysValidationError)?;

    let zomes_to_invoke = match op {
        Op::RegisterAgentActivity(RegisterAgentActivity { .. }) => ZomesToInvoke::AllIntegrity,
        Op::StoreRecord(StoreRecord { record }) => {
            let cascade = CascadeImpl::from_workspace_and_network(&workspace, network.clone());
            store_record_zomes_to_invoke(record.action(), ribosome, &cascade).await?
        }
        Op::StoreEntry(StoreEntry {
            action:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: action, ..
                        },
                    ..
                },
            ..
        }) => entry_creation_zomes_to_invoke(action, ribosome)?,
        Op::RegisterUpdate(RegisterUpdate {
            original_action, ..
        })
        | Op::RegisterDelete(RegisterDelete {
            original_action, ..
        }) => entry_creation_zomes_to_invoke(original_action, ribosome)?,
        Op::RegisterCreateLink(RegisterCreateLink {
            create_link:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: action, ..
                        },
                    ..
                },
            ..
        }) => create_link_zomes_to_invoke(action, ribosome)?,
        Op::RegisterDeleteLink(RegisterDeleteLink {
            create_link: action,
            ..
        }) => create_link_zomes_to_invoke(action, ribosome)?,
    };

    let invocation = ValidateInvocation::new(zomes_to_invoke, op)
        .map_err(|e| AppValidationError::RibosomeError(e.into()))?;
    let outcome = run_validation_callback_inner(
        invocation,
        ribosome,
        workspace,
        network.clone(),
        // (HashSet::<AnyDhtHash>::new(), 0),
        // HashSet::new(),
    )
    .await?;

    Ok(outcome)
}

/// Check the AppEntryDef is valid for the zome.
/// Check the EntryDefId and ZomeIndex are in range.
pub async fn check_entry_def(
    op: &Op,
    dna_hash: &DnaHash,
    conductor: &Conductor,
) -> SysValidationResult<()> {
    if let Some((_, EntryType::App(app_entry_def))) = op.entry_data() {
        check_app_entry_def(app_entry_def, dna_hash, conductor).await
    } else {
        Ok(())
    }
}

/// Check the AppEntryDef is valid for the zome.
/// Check the EntryDefId and ZomeIndex are in range.
pub async fn check_app_entry_def(
    app_entry_def: &AppEntryDef,
    dna_hash: &DnaHash,
    conductor: &Conductor,
) -> SysValidationResult<()> {
    // We want to be careful about holding locks open to the conductor api
    // so calls are made in blocks
    let ribosome = conductor
        .get_ribosome(dna_hash)
        .map_err(|_| SysValidationError::DnaMissing(dna_hash.clone()))?;

    // Check if the zome is found
    let zome = ribosome
        .get_integrity_zome(&app_entry_def.zome_index())
        .ok_or_else(|| ValidationOutcome::ZomeIndex(app_entry_def.clone()))?
        .into_inner()
        .1;

    let entry_def = get_entry_def(app_entry_def.entry_index(), zome, dna_hash, conductor).await?;

    // Check the visibility and return
    match entry_def {
        Some(entry_def) => {
            if entry_def.visibility == *app_entry_def.visibility() {
                Ok(())
            } else {
                Err(ValidationOutcome::EntryVisibility(app_entry_def.clone()).into())
            }
        }
        None => Err(ValidationOutcome::EntryDefId(app_entry_def.clone()).into()),
    }
}

pub fn entry_creation_zomes_to_invoke(
    action: &EntryCreationAction,
    ribosome: &impl RibosomeT,
) -> AppValidationOutcome<ZomesToInvoke> {
    match action {
        EntryCreationAction::Create(Create {
            entry_type: EntryType::App(app_entry_def),
            ..
        })
        | EntryCreationAction::Update(Update {
            entry_type: EntryType::App(app_entry_def),
            ..
        }) => {
            let zome = ribosome
                .get_integrity_zome(&app_entry_def.zome_index())
                .ok_or_else(|| {
                    Outcome::rejected(format!(
                        "Zome does not exist for {:?}",
                        app_entry_def.zome_index()
                    ))
                })?;
            Ok(ZomesToInvoke::OneIntegrity(zome))
        }
        _ => Ok(ZomesToInvoke::AllIntegrity),
    }
}

fn create_link_zomes_to_invoke(
    create_link: &CreateLink,
    ribosome: &impl RibosomeT,
) -> AppValidationOutcome<ZomesToInvoke> {
    let zome = ribosome
        .get_integrity_zome(&create_link.zome_index)
        .ok_or_else(|| {
            Outcome::rejected(format!(
                "Zome does not exist for {:?}",
                create_link.link_type
            ))
        })?;
    Ok(ZomesToInvoke::One(zome.erase_type()))
}

/// Get the zomes to invoke for an [`Op::StoreRecord`].
async fn store_record_zomes_to_invoke(
    action: &Action,
    ribosome: &impl RibosomeT,
    cascade: &(impl Cascade + Send + Sync),
) -> AppValidationOutcome<ZomesToInvoke> {
    // For deletes there is no entry type to check, so we get the previous action to see if that
    // was a create or a delete for an app entry type.
    let action = match action {
        Action::Delete(Delete {
            deletes_address, ..
        })
        | Action::DeleteLink(DeleteLink {
            link_add_address: deletes_address,
            ..
        }) => {
            let (deletes_action, _) = cascade
                .retrieve_action(deletes_address.clone(), NetworkGetOptions::default())
                .await?
                .ok_or_else(|| Outcome::awaiting(deletes_address))?;

            deletes_action.action().clone()
        }
        _ => action.clone(),
    };

    match action {
        Action::CreateLink(create_link) => create_link_zomes_to_invoke(&create_link, ribosome),
        Action::Create(Create {
            entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
            ..
        })
        | Action::Update(Update {
            entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
            ..
        }) => {
            let zome = ribosome.get_integrity_zome(&zome_index).ok_or_else(|| {
                Outcome::rejected(format!("Zome does not exist for {:?}", zome_index))
            })?;
            Ok(ZomesToInvoke::OneIntegrity(zome))
        }
        _ => Ok(ZomesToInvoke::AllIntegrity),
    }
}

async fn run_validation_callback_inner<R>(
    invocation: ValidateInvocation,
    ribosome: &R,
    workspace_read: HostFnWorkspaceRead,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome>
where
    R: RibosomeT,
{
    let validate_result = ribosome.run_validate(
        ValidateHostAccess::new(workspace_read.clone(), network.clone()),
        invocation.clone(),
    )?;
    match &validate_result {
        ValidateResult::Valid => (),
        _ => tracing::error!("validate result {validate_result:?}"),
    }
    match validate_result {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(hashes)) => {
            tracing::error!("got unresolved deps {hashes:?}");
            hashes.clone().into_iter().for_each(|hash| {
                let cascade_workspace = workspace_read.clone();
                tokio::spawn({
                    let cascade = CascadeImpl::from_workspace_and_network(
                        &cascade_workspace,
                        network.clone(),
                    );
                    async move {
                        let result = cascade
                            .fetch_record(hash.clone(), NetworkGetOptions::must_get_options())
                            .await;
                        tracing::error!("fetch_record result is {result:?}");
                    }
                });
            });
            Ok(Outcome::AwaitingDeps(hashes))
        }
        ValidateResult::UnresolvedDependencies(UnresolvedDependencies::AgentActivity(
            author,
            filter,
        )) => {
            tracing::error!("got unresolved deps agent activity {author:?} {filter:?}");
            let cascade_workspace = workspace_read.clone();
            tokio::spawn({
                let author = author.clone();
                let cascade =
                    CascadeImpl::from_workspace_and_network(&cascade_workspace, network.clone());
                async move {
                    let result = cascade.must_get_agent_activity(author, filter).await;
                    tracing::error!("must_get_agent_activity result is {result:?}");
                }
            });
            Ok(Outcome::AwaitingDeps(vec![author.into()]))
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
    ) -> CascadeImpl<Network> {
        CascadeImpl::empty()
            .with_authored(self.authored_db.clone())
            .with_dht(self.dht_db.clone().into())
            .with_network(network, self.cache.clone())
    }
}

pub fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStage,
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
    set_validation_stage(txn, hash, ValidationStage::AwaitingIntegration)?;
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
    set_validation_stage(txn, hash, ValidationStage::Pending)?;
    set_when_integrated(txn, hash, Timestamp::now())?;

    // If the op is rejected then force a receipt to be processed because the
    // receipt is a warrant, so of course the author won't want it to be
    // produced.
    if matches!(status, ValidationStatus::Rejected) {
        set_require_receipt(txn, hash, true)?;
    }
    Ok(())
}
