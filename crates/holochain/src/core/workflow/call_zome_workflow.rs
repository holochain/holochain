use super::app_validation_workflow;
use super::app_validation_workflow::Outcome;
use super::error::WorkflowResult;
use super::sys_validation_workflow::sys_validate_element;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::post_commit::send_post_commit;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::workflow::error::WorkflowError;
use either::Either;
use holochain_cascade::Cascade;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::source_chain::SourceChainError;
use holochain_zome_types::element::Element;

use holochain_types::prelude::*;
use std::sync::Arc;
use tracing::instrument;

#[cfg(test)]
mod validation_test;

/// Placeholder for the return value of a zome invocation
pub type ZomeCallResult = RibosomeResult<ZomeCallResponse>;

pub struct CallZomeWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + Send,
{
    pub ribosome: Ribosome,
    pub invocation: ZomeCallInvocation,
    pub signal_tx: SignalBroadcaster,
    pub conductor_handle: ConductorHandle,
    pub is_root_zome_call: bool,
    pub cell_id: CellId,
}

#[instrument(skip(
    workspace,
    network,
    keystore,
    args,
    trigger_publish_dht_ops,
    trigger_integrate_dht_ops
))]
pub async fn call_zome_workflow<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: CallZomeWorkflowArgs<Ribosome>,
    trigger_publish_dht_ops: TriggerSender,
    trigger_integrate_dht_ops: TriggerSender,
) -> WorkflowResult<ZomeCallResult>
where
    Ribosome: RibosomeT + Send + 'static,
{
    let should_write = args.is_root_zome_call;
    let conductor_handle = args.conductor_handle.clone();
    let result =
        call_zome_workflow_inner(workspace.clone(), network.clone(), keystore.clone(), args)
            .await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    if should_write {
        let is_empty = workspace.source_chain().is_empty()?;
        let countersigning_op = workspace.source_chain().countersigning_op()?;
        let flushed_headers: Vec<(Option<Zome>, SignedHeaderHashed)> =
            HostFnWorkspace::from(workspace.clone())
                .flush(&network)
                .await?;
        if !is_empty {
            match countersigning_op {
                Some(op) => {
                    if let Err(error_response) =
                        super::countersigning_workflow::countersigning_publish(&network, op).await
                    {
                        return Ok(Ok(error_response));
                    }
                }
                None => {
                    trigger_publish_dht_ops.trigger();
                    trigger_integrate_dht_ops.trigger();
                }
            }
        }

        send_post_commit(
            conductor_handle,
            workspace,
            network,
            keystore,
            flushed_headers,
        )
        .await?;
    }

    Ok(result)
}

async fn call_zome_workflow_inner<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: CallZomeWorkflowArgs<Ribosome>,
) -> WorkflowResult<ZomeCallResult>
where
    Ribosome: RibosomeT + Send + 'static,
{
    let CallZomeWorkflowArgs {
        ribosome,
        invocation,
        signal_tx,
        conductor_handle,
        cell_id,
        ..
    } = args;

    let call_zome_handle =
        CellConductorApi::new(conductor_handle.clone(), cell_id).into_call_zome_handle();
    let zome = invocation.zome.clone();

    tracing::trace!("Before zome call");
    let host_access = ZomeCallHostAccess::new(
        workspace.clone().into(),
        keystore,
        network.clone(),
        signal_tx,
        call_zome_handle,
    );
    let (ribosome, result) =
        call_zome_function_authorized(ribosome, host_access, invocation).await?;
    tracing::trace!("After zome call");

    let validation_result = inline_validation(
        workspace.clone(),
        network,
        conductor_handle,
        Some(zome),
        ribosome,
    )
    .await;
    if matches!(
        validation_result,
        Err(WorkflowError::SourceChainError(
            SourceChainError::InvalidCommit(_)
        ))
    ) {
        let scratch_elements = workspace.source_chain().scratch_elements()?;
        if scratch_elements.len() == 1 {
            let lock = holochain_state::source_chain::lock_for_entry(
                scratch_elements[0].entry().as_option(),
            )?;
            if !lock.is_empty()
                && workspace
                    .source_chain()
                    .is_chain_locked(Vec::with_capacity(0))
                    .await?
                && !workspace.source_chain().is_chain_locked(lock).await?
            {
                if let Err(error) = workspace.source_chain().unlock_chain().await {
                    tracing::error!(?error);
                }
            }
        }
    }
    validation_result?;
    Ok(result)
}

/// First check if we are authorized to call
/// the zome function.
/// Then send to a background thread and
/// call the zome function.
pub async fn call_zome_function_authorized<R>(
    ribosome: R,
    host_access: ZomeCallHostAccess,
    invocation: ZomeCallInvocation,
) -> WorkflowResult<(R, RibosomeResult<ZomeCallResponse>)>
where
    R: RibosomeT + Send + 'static,
{
    if invocation.is_authorized(&host_access).await? {
        tokio::task::spawn_blocking(|| {
            let r = ribosome.call_zome_function(host_access, invocation);
            Ok((ribosome, r))
        })
        .await?
    } else {
        Ok((
            ribosome,
            Ok(ZomeCallResponse::Unauthorized(
                invocation.cell_id.clone(),
                invocation.zome.zome_name().clone(),
                invocation.fn_name.clone(),
                invocation.provenance.clone(),
            )),
        ))
    }
}
/// Run validation inline and wait for the result.
pub async fn inline_validation<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
    zome: Option<Zome>,
    ribosome: Ribosome,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + Send + 'static,
{
    let to_app_validate = {
        // collect all the elements we need to validate in wasm
        let scratch_elements = workspace.source_chain().scratch_elements()?;
        let mut to_app_validate: Vec<Element> = Vec::with_capacity(scratch_elements.len());
        // Loop forwards through all the new elements
        for element in scratch_elements {
            sys_validate_element(&element, &workspace, network.clone(), &(*conductor_handle))
                .await
                // If the was en error exit
                // If the validation failed, exit with an InvalidCommit
                // If it was ok continue
                .or_else(|outcome_or_err| outcome_or_err.invalid_call_zome_commit())?;
            to_app_validate.push(element);
        }

        to_app_validate
    };

    {
        for chain_element in to_app_validate {
            let zome = match zome.clone() {
                Some(zome) => ZomesToInvoke::One(zome),
                None => {
                    get_zome(
                        &chain_element,
                        &workspace,
                        network.clone(),
                        ribosome.dna_def(),
                    )
                    .await?
                }
            };
            let outcome = match chain_element.header() {
                Header::Dna(_)
                | Header::AgentValidationPkg(_)
                | Header::OpenChain(_)
                | Header::CloseChain(_)
                | Header::InitZomesComplete(_) => {
                    // These headers don't get validated
                    continue;
                }
                Header::CreateLink(link_add) => {
                    let (base, target) = {
                        let mut cascade = holochain_cascade::Cascade::from_workspace_network(
                            &workspace,
                            network.clone(),
                        );
                        let base_address = &link_add.base_address;
                        let base = cascade
                            .retrieve_entry(base_address.clone(), Default::default())
                            .await
                            .map_err(RibosomeError::from)?
                            .ok_or_else(|| RibosomeError::ElementDeps(base_address.clone().into()))?
                            .into_content();
                        let base = Arc::new(base);

                        let target_address = &link_add.target_address;
                        let target = cascade
                            .retrieve_entry(target_address.clone(), Default::default())
                            .await
                            .map_err(RibosomeError::from)?
                            .ok_or_else(|| {
                                RibosomeError::ElementDeps(target_address.clone().into())
                            })?
                            .into_content();
                        let target = Arc::new(target);
                        (base, target)
                    };
                    let link_add = Arc::new(link_add.clone());

                    Either::Left(
                        app_validation_workflow::run_create_link_validation_callback(
                            app_validation_workflow::to_single_zome(zome)?,
                            link_add,
                            base,
                            target,
                            &ribosome,
                            workspace.clone(),
                            network.clone(),
                        )?,
                    )
                }
                Header::DeleteLink(delete_link) => Either::Left(
                    app_validation_workflow::run_delete_link_validation_callback(
                        app_validation_workflow::to_single_zome(zome)?,
                        delete_link.clone(),
                        &ribosome,
                        workspace.clone(),
                        network.clone(),
                    )?,
                ),
                Header::Create(_) | Header::Update(_) | Header::Delete(_) => Either::Right(
                    app_validation_workflow::run_validation_callback_direct(
                        zome,
                        chain_element,
                        &ribosome,
                        workspace.clone(),
                        network.clone(),
                        &conductor_handle,
                    )
                    .await?,
                ),
            };
            map_outcome(outcome)?;
        }
    }
    Ok(())
}

fn map_outcome(outcome: Either<app_validation_workflow::Outcome, Outcome>) -> WorkflowResult<()> {
    match outcome {
        Either::Left(outcome) => match outcome {
            app_validation_workflow::Outcome::Accepted => {}
            app_validation_workflow::Outcome::Rejected(reason) => {
                return Err(SourceChainError::InvalidLink(reason).into());
            }
            app_validation_workflow::Outcome::AwaitingDeps(hashes) => {
                return Err(SourceChainError::InvalidCommit(format!("{:?}", hashes)).into());
            }
        },
        Either::Right(outcome) => match outcome {
            app_validation_workflow::Outcome::Accepted => {}
            app_validation_workflow::Outcome::Rejected(reason) => {
                return Err(SourceChainError::InvalidCommit(reason).into());
            }
            // when the wasm is being called directly in a zome invocation any
            // state other than valid is not allowed for new entries
            // e.g. we require that all dependencies are met when committing an
            // entry to a local source chain
            // this is different to the case where we are validating data coming in
            // from the network where unmet dependencies would need to be
            // rescheduled to attempt later due to partitions etc.
            app_validation_workflow::Outcome::AwaitingDeps(hashes) => {
                return Err(SourceChainError::InvalidCommit(format!("{:?}", hashes)).into());
            }
        },
    }
    Ok(())
}
async fn get_zome(
    element: &Element,
    workspace: &SourceChainWorkspace,
    network: HolochainP2pDna,
    dna_def: &DnaDefHashed,
) -> WorkflowResult<crate::core::ribosome::ZomesToInvoke> {
    let mut cascade = Cascade::from_workspace_network(workspace, network);
    let result = app_validation_workflow::get_zomes_to_invoke(element, dna_def, &mut cascade).await;
    match result {
        Ok(zomes) => Ok(zomes),
        Err(outcome_or_err) => {
            let outcome = outcome_or_err.try_into()?;
            match outcome {
                app_validation_workflow::Outcome::AwaitingDeps(hashes) => {
                    return Err(SourceChainError::InvalidCommit(format!("{:?}", hashes)).into());
                }
                _ => unreachable!("get_zomes_to_invoke only returns success, error or await"),
            }
        }
    }
}
