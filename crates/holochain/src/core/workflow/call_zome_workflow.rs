use super::app_validation_workflow;
use super::app_validation_workflow::AppValidationError;
use super::app_validation_workflow::Outcome;
use super::error::WorkflowResult;
use super::sys_validation_workflow::sys_validate_element;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::post_commit::send_post_commit;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::error::WorkflowError;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::source_chain::SourceChainError;
use holochain_zome_types::element::Element;

use holochain_types::prelude::*;
use tracing::instrument;

#[cfg(test)]
mod validation_test;

/// Placeholder for the return value of a zome invocation
pub type ZomeCallResult = RibosomeResult<ZomeCallResponse>;

pub struct CallZomeWorkflowArgs<RibosomeT> {
    pub ribosome: RibosomeT,
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
    Ribosome: RibosomeT + 'static,
{
    let coordinator_zome = args
        .ribosome
        .dna_def()
        .get_coordinator_zome(args.invocation.zome.zome_name())
        .ok();
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
        let flushed_actions: Vec<SignedActionHashed> = HostFnWorkspace::from(workspace.clone())
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
                    trigger_publish_dht_ops.trigger(&"trigger_publish_dht_ops");
                    trigger_integrate_dht_ops.trigger(&"trigger_integrate_dht_ops");
                }
            }
        }

        // Only send post commit if this is a coordinator zome.
        if let Some(coordinator_zome) = coordinator_zome {
            send_post_commit(
                conductor_handle,
                workspace,
                network,
                keystore,
                flushed_actions,
                vec![coordinator_zome],
            )
            .await?;
        }
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
    Ribosome: RibosomeT + 'static,
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

    let validation_result =
        inline_validation(workspace.clone(), network, conductor_handle, ribosome).await;
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
    R: RibosomeT + 'static,
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
    ribosome: Ribosome,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + 'static,
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

    let mut cascade =
        holochain_cascade::Cascade::from_workspace_network(&workspace, network.clone());
    for mut chain_element in to_app_validate {
        for op_type in action_to_op_types(chain_element.action()) {
            let op =
                app_validation_workflow::element_to_op(chain_element, op_type, &mut cascade).await;

            let (op, activity_entry) = match op {
                Ok(op) => op,
                Err(outcome_or_err) => return map_outcome(Outcome::try_from(outcome_or_err)),
            };

            let outcome = app_validation_workflow::validate_op(
                &op,
                workspace.clone().into(),
                &network,
                &ribosome,
            )
            .await;
            let outcome = outcome.or_else(Outcome::try_from);
            map_outcome(outcome)?;
            chain_element = app_validation_workflow::op_to_element(op, activity_entry);
        }
    }

    Ok(())
}

fn map_outcome(
    outcome: Result<app_validation_workflow::Outcome, AppValidationError>,
) -> WorkflowResult<()> {
    match outcome.map_err(SourceChainError::other)? {
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
    }
    Ok(())
}
