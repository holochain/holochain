//! The workflow and queue consumer for sys validation

use std::convert::TryInto;
use std::sync::Arc;

use self::validation_package::get_as_author_custom;
use self::validation_package::get_as_author_full;
use self::validation_package::get_as_author_sub_chain;

use super::error::WorkflowResult;
use super::sys_validation_workflow::validation_query;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::entry_def_store::get_entry_def;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::guest_callback::validate_link::ValidateCreateLinkInvocation;
use crate::core::ribosome::guest_callback::validate_link::ValidateDeleteLinkInvocation;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkHostAccess;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkInvocation;
use crate::core::ribosome::guest_callback::validate_link::ValidateLinkResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageHostAccess;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomesToInvoke;
use error::AppValidationResult;
pub use error::*;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use rusqlite::Transaction;
use tracing::*;
pub use types::Outcome;

#[cfg(todo_redo_old_tests)]
mod network_call_tests;
#[cfg(test)]
mod tests;

mod error;
mod types;
pub mod validation_package;

const NUM_CONCURRENT_OPS: usize = 50;

#[instrument(skip(workspace, trigger_integration, conductor_api, network))]
pub async fn app_validation_workflow(
    workspace: AppValidationWorkspace,
    trigger_integration: TriggerSender,
    network: HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let complete =
        app_validation_workflow_inner(Arc::new(workspace), conductor_api, &network).await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // trigger other workflows
    trigger_integration.trigger();

    Ok(complete)
}

async fn app_validation_workflow_inner(
    workspace: Arc<AppValidationWorkspace>,
    network: &HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let env = workspace.vault.clone().into();
    let sorted_ops = validation_query::get_ops_to_app_validate(&env).await?;
    let conductor_api = Arc::new(conductor_api);
    let this_agent = network.from_agent();

    // Validate all the ops
    let iter = sorted_ops.into_iter().map(|so| {
        let network = network.clone();
        let conductor_api = conductor_api.clone();
        let workspace = workspace.clone();
        async move {
            let (op, op_hash) = so.into_inner();
            let author = op.header().author().clone();
            let op_light = op.to_light();

            // Validate this op
            let r = validate_op(op, &(*conductor_api), &(*workspace), &network).await;
            (op_hash, author, op_light, r)
        }
    });
    use futures::stream::StreamExt;
    let mut iter = futures::stream::iter(iter)
        .buffer_unordered(NUM_CONCURRENT_OPS)
        .ready_chunks(NUM_CONCURRENT_OPS);

    while let Some(chunk) = iter.next().await {
        workspace
            .vault
            .async_commit(move |mut txn| {
                for outcome in chunk {
                    let (op_hash, author, op_light, outcome) = outcome;
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
                            put_integration_limbo(&mut txn, op_hash, ValidationStatus::Valid)?;
                        }
                        Outcome::AwaitingDeps(deps) => {
                            let status = ValidationLimboStatus::AwaitingAppDeps(deps);
                            put_validation_limbo(&mut txn, op_hash, status)?;
                        }
                        Outcome::Rejected(_) => {
                                tracing::warn!("Received invalid op! Warrants aren't implemented yet, so we can't do anything about this right now, but be warned that somebody on the network has maliciously hacked their node.\nOp: {:?}", op_light);
                            put_integration_limbo(&mut txn, op_hash, ValidationStatus::Rejected)?;
                        }
                    }
                }
                WorkflowResult::Ok(())
            })
            .await?;
    }
    Ok(WorkComplete::Complete)
}

pub fn to_single_zome(zomes_to_invoke: ZomesToInvoke) -> AppValidationResult<Zome> {
    match zomes_to_invoke {
        ZomesToInvoke::All => Err(AppValidationError::LinkMultipleZomes),
        ZomesToInvoke::One(z) => Ok(z),
    }
}

async fn validate_op(
    op: DhtOp,
    conductor_api: &impl CellConductorApiT,
    workspace: &AppValidationWorkspace,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Outcome> {
    // Get the workspace for the validation calls
    let workspace_lock = workspace.validation_workspace(network.from_agent()).await?;

    // Create the element
    let element = get_element(op)?;

    // Check for caps
    check_for_caps(&element)?;

    // Get the dna file
    let dna_file = conductor_api.get_this_dna();
    let dna_file =
        dna_file.map_err(|_| AppValidationError::DnaMissing(conductor_api.cell_id().clone()))?;

    // Get the EntryDefId associated with this Element if there is one
    let entry_def = {
        let mut cascade = workspace.full_cascade(network.clone());
        get_associated_entry_def(&element, dna_file.dna(), conductor_api, &mut cascade).await?
    };

    // Create the ribosome
    let ribosome = RealRibosome::new(dna_file);

    // Get the validation package
    let validation_package = get_validation_package(
        &element,
        &entry_def,
        Some(workspace),
        &ribosome,
        &workspace_lock,
        network,
    )
    .await?;

    // Get the EntryDefId associated with this Element if there is one
    let entry_def_id = entry_def.map(|ed| ed.id);

    // Get the zome names
    let mut cascade = workspace.full_cascade(network.clone());
    let zomes_to_invoke = get_zomes_to_invoke(&element, ribosome.dna_def(), &mut cascade).await?;

    let outcome = match element.header() {
        Header::DeleteLink(delete_link) => {
            let zome_name = to_single_zome(zomes_to_invoke)?;
            // Run the link validation
            run_delete_link_validation_callback(
                zome_name,
                delete_link.clone(),
                &ribosome,
                workspace_lock.clone(),
                network.clone(),
            )?
        }
        Header::CreateLink(link_add) => {
            // Get the base and target for this link
            let mut cascade = workspace.full_cascade(network.clone());
            let base = cascade
                .retrieve_entry(link_add.base_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&link_add.base_address))?;
            let target = cascade
                .retrieve_entry(link_add.target_address.clone(), Default::default())
                .await?
                .map(|e| e.into_content())
                .ok_or_else(|| Outcome::awaiting(&link_add.target_address))?;

            let link_add = Arc::new(link_add.clone());
            let base = Arc::new(base);
            let target = Arc::new(target);

            let zome_name = to_single_zome(zomes_to_invoke)?;

            // Run the link validation
            run_create_link_validation_callback(
                zome_name,
                link_add,
                base,
                target,
                &ribosome,
                workspace_lock.clone(),
                network.clone(),
            )?
        }
        _ => {
            // Element

            // Call the callback
            let element = Arc::new(element);
            let validation_package = validation_package.map(Arc::new);
            // Call the element validation
            run_validation_callback_inner(
                zomes_to_invoke,
                element,
                validation_package,
                entry_def_id,
                &ribosome,
                workspace_lock.clone(),
                network.clone(),
            )?
        }
    };
    Ok(outcome)
}

/// Get the [EntryDef] associated with this
/// element if there is one.
///
/// Create and Update will get the def from
/// the AppEntryType on their header.
///
/// Delete will get the def from the
/// header on the `deletes_address` field.
///
/// Other header types will None.
async fn get_associated_entry_def(
    element: &Element,
    dna_def: &DnaDefHashed,
    conductor_api: &impl CellConductorApiT,
    cascade: &mut Cascade,
) -> AppValidationOutcome<Option<EntryDef>> {
    match get_app_entry_type(element, cascade).await? {
        Some(aet) => {
            let zome = get_zome_info(&aet, dna_def)?.1.clone();
            Ok(get_entry_def(aet.id(), zome, dna_def, conductor_api).await?)
        }
        None => Ok(None),
    }
}

/// Get the element from the op or
/// return accepted because we don't app
/// validate this op.
fn get_element(op: DhtOp) -> AppValidationOutcome<Element> {
    match op {
        DhtOp::RegisterAgentActivity(_, _) => Outcome::accepted(),
        DhtOp::StoreElement(s, h, e) => match h {
            Header::Delete(_) | Header::CreateLink(_) | Header::DeleteLink(_) => Ok(Element::new(
                SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h), s),
                None,
            )),
            Header::Update(_) | Header::Create(_) => Ok(Element::new(
                SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h), s),
                e.map(|e| *e),
            )),
            _ => Outcome::accepted(),
        },
        DhtOp::StoreEntry(s, h, e) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            Some(*e),
        )),
        DhtOp::RegisterUpdatedContent(s, h, e) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            e.map(|e| *e),
        )),
        DhtOp::RegisterUpdatedElement(s, h, e) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            e.map(|e| *e),
        )),
        DhtOp::RegisterDeletedEntryHeader(s, h) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterDeletedBy(s, h) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterAddLink(s, h) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterRemoveLink(s, h) => Ok(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
    }
}

/// Check for capability headers
/// and exit as we don't want to validate them
fn check_for_caps(element: &Element) -> AppValidationOutcome<()> {
    match element.header().entry_type() {
        Some(EntryType::CapClaim) | Some(EntryType::CapGrant) => Outcome::accepted(),
        _ => Ok(()),
    }
}

/// Get the zome name from the app entry type
/// or get all zome names.
pub async fn get_zomes_to_invoke(
    element: &Element,
    dna_def: &DnaDef,
    cascade: &mut Cascade,
) -> AppValidationOutcome<ZomesToInvoke> {
    let aet = { get_app_entry_type(element, cascade).await? };
    match aet {
        Some(aet) => Ok(ZomesToInvoke::One(get_zome(&aet, dna_def)?)),
        None => match element.header() {
            Header::CreateLink(_) | Header::DeleteLink(_) => {
                get_link_zome(element, dna_def, cascade).await
            }
            _ => Ok(ZomesToInvoke::All),
        },
    }
}

fn get_zome_info<'a>(
    entry_type: &AppEntryType,
    dna_def: &'a DnaDef,
) -> AppValidationResult<&'a (ZomeName, ZomeDef)> {
    let zome_index = u8::from(entry_type.zome_id()) as usize;
    dna_def
        .zomes
        .get(zome_index)
        .ok_or_else(|| AppValidationError::ZomeId(entry_type.zome_id()))
}

fn get_zome(entry_type: &AppEntryType, dna_def: &DnaDef) -> AppValidationResult<Zome> {
    zome_id_to_zome(entry_type.zome_id(), dna_def)
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

/// Either get the app entry type
/// from this entry or from the dependency.
async fn get_app_entry_type(
    element: &Element,
    cascade: &mut Cascade,
) -> AppValidationOutcome<Option<AppEntryType>> {
    match element.header().entry_data() {
        Some((_, et)) => match et.clone() {
            EntryType::App(aet) => Ok(Some(aet)),
            EntryType::AgentPubKey | EntryType::CapClaim | EntryType::CapGrant => Ok(None),
        },
        None => get_app_entry_type_from_dep(element, cascade).await,
    }
}

async fn get_link_zome(
    element: &Element,
    dna_def: &DnaDef,
    cascade: &mut Cascade,
) -> AppValidationOutcome<ZomesToInvoke> {
    match element.header() {
        Header::CreateLink(cl) => {
            let zome = zome_id_to_zome(cl.zome_id, dna_def)?;
            Ok(ZomesToInvoke::One(zome))
        }
        Header::DeleteLink(dl) => {
            let shh = cascade
                .retrieve_header(dl.link_add_address.clone(), Default::default())
                .await?
                .ok_or_else(|| Outcome::awaiting(&dl.link_add_address))?;

            match shh.header() {
                Header::CreateLink(cl) => {
                    let zome = zome_id_to_zome(cl.zome_id, dna_def)?;
                    Ok(ZomesToInvoke::One(zome))
                }
                // The header that was found was the wrong type
                // so lets try again.
                _ => Err(Outcome::awaiting(&dl.link_add_address)),
            }
        }
        _ => unreachable!(),
    }
}

/// Retrieve the dependency and extract
/// the app entry type so we know which zome to call
async fn get_app_entry_type_from_dep(
    element: &Element,
    cascade: &mut Cascade,
) -> AppValidationOutcome<Option<AppEntryType>> {
    match element.header() {
        Header::Delete(ed) => {
            let el = cascade
                .retrieve(ed.deletes_address.clone().into(), Default::default())
                .await?
                .ok_or_else(|| Outcome::awaiting(&ed.deletes_address))?;
            Ok(extract_app_type(&el))
        }
        _ => Ok(None),
    }
}

fn extract_app_type(element: &Element) -> Option<AppEntryType> {
    element
        .header()
        .entry_data()
        .and_then(|(_, entry_type)| match entry_type {
            EntryType::App(aet) => Some(aet.clone()),
            _ => None,
        })
}

/// Get the validation package based on
/// the requirements set by the AppEntryType
async fn get_validation_package(
    element: &Element,
    entry_def: &Option<EntryDef>,
    // from_agent: Option<AgentPubKey>,
    workspace: Option<&AppValidationWorkspace>,
    ribosome: &impl RibosomeT,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Option<ValidationPackage>> {
    match entry_def {
        Some(entry_def) => match workspace {
            Some(workspace) => {
                get_validation_package_remote(
                    element,
                    entry_def,
                    // from_agent,
                    workspace,
                    ribosome,
                    workspace_lock,
                    network,
                )
                .await
            }
            None => {
                get_validation_package_local(
                    element,
                    entry_def.required_validation_type,
                    ribosome,
                    workspace_lock,
                    network,
                )
                .await
            }
        },
        None => {
            // Not an entry header type so no package
            Ok(None)
        }
    }
}

async fn get_validation_package_local(
    element: &Element,
    required_validation_type: RequiredValidationType,
    ribosome: &impl RibosomeT,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Option<ValidationPackage>> {
    let header_seq = element.header().header_seq();
    match required_validation_type {
        RequiredValidationType::Element => Ok(None),
        RequiredValidationType::SubChain => {
            let app_entry_type = match element.header().entry_type().cloned() {
                Some(EntryType::App(aet)) => aet,
                _ => return Ok(None),
            };
            Ok(Some(
                get_as_author_sub_chain(header_seq, app_entry_type, workspace_lock.source_chain())
                    .await?,
            ))
        }
        RequiredValidationType::Full => Ok(Some(
            get_as_author_full(header_seq, workspace_lock.source_chain()).await?,
        )),
        RequiredValidationType::Custom => {
            {
                let cascade = Cascade::from_workspace(workspace_lock);
                if let Some(elements) =
                    cascade.get_validation_package_local(element.header_address())?
                {
                    return Ok(Some(ValidationPackage::new(elements)));
                }
            }
            let result = match get_as_author_custom(
                element.header_hashed(),
                ribosome,
                network,
                workspace_lock.clone(),
            )? {
                Some(result) => result,
                None => return Ok(None),
            };
            match result {
                ValidationPackageResult::Success(validation_package) => {
                    Ok(Some(validation_package))
                }
                ValidationPackageResult::Fail(reason) => Outcome::exit_with_rejected(reason),
                ValidationPackageResult::UnresolvedDependencies(deps) => {
                    Outcome::exit_with_awaiting(deps)
                }
                ValidationPackageResult::NotImplemented => Outcome::exit_with_rejected(format!(
                    "Entry definition specifies a custom validation package but the callback isn't defined for {:?}",
                    element
                )),
            }
        }
    }
}

async fn get_validation_package_remote(
    element: &Element,
    entry_def: &EntryDef,
    // from_agent: Option<AgentPubKey>,
    workspace: &AppValidationWorkspace,
    ribosome: &impl RibosomeT,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Option<ValidationPackage>> {
    match entry_def.required_validation_type {
        // Only needs the element
        RequiredValidationType::Element => Ok(None),
        RequiredValidationType::SubChain | RequiredValidationType::Full => {
            let agent_id = element.header().author().clone();
            {
                let mut cascade = workspace.full_cascade(network.clone());
                // Get from author
                let header_hashed = element.header_hashed();
                if let Some(validation_package) = cascade
                    .get_validation_package(agent_id.clone(), header_hashed)
                    .await?
                {
                    return Ok(Some(validation_package));
                }

                // TODO: When we implement validation package then we might need this again.
                // Fallback to gossiper if author is unavailable
                // if let Some(from_agent) = from_agent {
                //     if let Some(validation_package) = cascade
                //         .get_validation_package(from_agent, header_hashed)
                //         .await?
                //     {
                //         return Ok(Some(validation_package));
                //     }
                // }
            }

            // Fallback to RegisterAgentActivity if gossiper is unavailable

            // When getting agent activity we need to get all the elements from element authorities
            // in parallel but if the network is small this could overwhelm the authorities and we
            // might need to retry some of the gets.
            // One consequence of this is the max timeout becomes the network timeout * NUM_RETRY_GETS
            // if the data really isn't available.
            // TODO: Another solution is to up the timeout for parallel gets.
            const NUM_RETRY_GETS: u8 = 3;
            let range = 0..element.header().header_seq().saturating_sub(1);

            let mut query = holochain_zome_types::query::ChainQueryFilter::new()
                .sequence_range(range)
                .include_entries(true);
            if let (RequiredValidationType::SubChain, Some(et)) = (
                entry_def.required_validation_type,
                element.header().entry_type(),
            ) {
                query = query.entry_type(et.clone());
            }

            // Get the activity from the agent authority
            let options = GetActivityOptions {
                include_full_headers: true,
                include_valid_activity: true,
                retry_gets: NUM_RETRY_GETS,
                ..Default::default()
            };
            let activity = {
                let mut cascade = workspace.full_cascade(network.clone());
                cascade.get_agent_activity(agent_id, query, options).await?
            };
            match activity {
                AgentActivityResponse {
                    status: ChainStatus::Valid(_),
                    valid_activity: ChainItems::Full(elements),
                    ..
                } => {
                    // TODO: Are we going to cache validation packages?
                    // Add this back in when we implement validation packages.
                    // Cache this as a validation package
                    // workspace.meta_cache.register_validation_package(
                    //     element.header_address(),
                    //     elements.iter().map(|el| el.header_address().clone()),
                    // );
                    Ok(Some(ValidationPackage::new(elements)))
                }
                // TODO: If the chain is invalid should we still return
                // it as the validation package?
                _ => Ok(None),
            }
        }
        RequiredValidationType::Custom => {
            let validation_package = {
                let mut cascade = workspace.full_cascade(network.clone());
                let agent_id = element.header().author().clone();
                let header_hashed = element.header_hashed();
                // Call the author
                let validation_package = cascade
                    .get_validation_package(agent_id, header_hashed)
                    .await?;

                // Fallback to gossiper
                match &validation_package {
                    Some(_) => validation_package,
                    None => {
                        // TODO: When we implement validation package then we might need this again.
                        // if let Some(from_agent) = from_agent {
                        //     cascade
                        //         .get_validation_package(from_agent, header_hashed)
                        //         .await?
                        // } else {
                        //     None
                        // }
                        None
                    }
                }
            };

            // Fallback to callback
            match &validation_package {
                Some(_) => Ok(validation_package),
                None => {
                    let access =
                        ValidationPackageHostAccess::new(workspace_lock.clone(), network.clone());
                    let app_entry_type = match element.header().entry_type() {
                        Some(EntryType::App(a)) => a.clone(),
                        _ => return Ok(None),
                    };
                    let zome: Zome = ribosome
                        .dna_def()
                        .zomes
                        .get(app_entry_type.zome_id().index())
                        .ok_or_else(|| AppValidationError::ZomeId(app_entry_type.zome_id()))?
                        .clone()
                        .into();
                    let invocation = ValidationPackageInvocation::new(zome, app_entry_type);
                    match ribosome.run_validation_package(access, invocation)? {
                        ValidationPackageResult::Success(validation_package) => {
                            Ok(Some(validation_package))
                        }
                        ValidationPackageResult::Fail(reason) => {
                            Outcome::exit_with_rejected(reason)
                        }
                        ValidationPackageResult::UnresolvedDependencies(deps) => {
                            Outcome::exit_with_awaiting(deps)
                        }
                        ValidationPackageResult::NotImplemented => {
                            Outcome::exit_with_rejected(format!(
                                "Entry definition specifies a custom validation package but the callback isn't defined for {:?}",
                                element
                            ))
                        }
                    }
                }
            }
        }
    }
}

pub async fn run_validation_callback_direct(
    zome: ZomesToInvoke,
    element: Element,
    ribosome: &impl RibosomeT,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let outcome = {
        let mut cascade = Cascade::from_workspace_network(&workspace, network.clone());
        get_associated_entry_def(&element, ribosome.dna_def(), conductor_api, &mut cascade).await
    };

    // The outcome could be awaiting a dependency to get the entry def
    // so we need to check that here and exit early if that is the case
    let entry_def = match outcome {
        Ok(ed) => ed,
        Err(outcome) => return outcome.try_into(),
    };

    let validation_package = {
        let outcome = get_validation_package(
            &element, &entry_def, // None,
            None, ribosome, &workspace, &network,
        )
        .await;
        match outcome {
            Ok(vp) => vp.map(Arc::new),
            Err(outcome) => return outcome.try_into(),
        }
    };
    let entry_def_id = entry_def.map(|ed| ed.id);

    let element = Arc::new(element);

    run_validation_callback_inner(
        zome,
        element,
        validation_package,
        entry_def_id,
        ribosome,
        workspace,
        network,
    )
}

fn run_validation_callback_inner(
    zomes_to_invoke: ZomesToInvoke,
    element: Arc<Element>,
    validation_package: Option<Arc<ValidationPackage>>,
    entry_def_id: Option<EntryDefId>,
    ribosome: &impl RibosomeT,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let validate: ValidateResult = ribosome.run_validate(
        ValidateHostAccess::new(workspace_lock, network),
        ValidateInvocation {
            zomes_to_invoke,
            element,
            validation_package,
            entry_def_id,
        },
    )?;
    match validate {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(hashes) => Ok(Outcome::AwaitingDeps(hashes)),
    }
}

pub fn run_create_link_validation_callback(
    zome: Zome,
    link_add: Arc<CreateLink>,
    base: Arc<Entry>,
    target: Arc<Entry>,
    ribosome: &impl RibosomeT,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let invocation = ValidateCreateLinkInvocation {
        zome,
        link_add,
        base,
        target,
    };
    let invocation = ValidateLinkInvocation::<ValidateCreateLinkInvocation>::new(invocation);
    run_link_validation_callback(invocation, ribosome, workspace_lock, network)
}

pub fn run_delete_link_validation_callback(
    zome: Zome,
    delete_link: DeleteLink,
    ribosome: &impl RibosomeT,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let invocation = ValidateDeleteLinkInvocation { zome, delete_link };
    let invocation = ValidateLinkInvocation::<ValidateDeleteLinkInvocation>::new(invocation);
    run_link_validation_callback(invocation, ribosome, workspace_lock, network)
}

pub fn run_link_validation_callback<I: Invocation + 'static>(
    invocation: ValidateLinkInvocation<I>,
    ribosome: &impl RibosomeT,
    network: HolochainP2pDna,
) -> AppValidationResult<Outcome> {
    let access = ValidateLinkHostAccess::new(workspace_lock, network);
    let validate = ribosome.run_validate_link(access, invocation)?;
    match validate {
        ValidateLinkResult::Valid => Ok(Outcome::Accepted),
        ValidateLinkResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateLinkResult::UnresolvedDependencies(hashes) => Ok(Outcome::AwaitingDeps(hashes)),
    }
}

pub struct AppValidationWorkspace {
    vault: EnvWrite,
    cache: EnvWrite,
}

impl AppValidationWorkspace {
    pub fn new(vault: EnvWrite, cache: EnvWrite) -> Self {
        Self { vault, cache }
    }

    pub async fn validation_workspace(
        &self,
        author: AgentPubKey,
    ) -> AppValidationResult<HostFnWorkspace> {
        Ok(HostFnWorkspace::new(self.vault.clone(), self.cache.clone(), author).await?)
    }

    pub fn full_cascade<Network: HolochainP2pDnaT + Clone + 'static + Send>(
        &self,
        network: Network,
    ) -> Cascade<Network> {
        Cascade::empty()
            .with_vault(self.vault.clone().into())
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

impl From<&HostFnWorkspace> for AppValidationWorkspace {
    fn from(h: &HostFnWorkspace) -> Self {
        let (vault, cache) = h.databases();
        Self {
            vault: vault.into(),
            cache,
        }
    }
}
