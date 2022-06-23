use super::*;
use tracing::*;

#[instrument(skip(
    workspace,
    space,
    trigger_app_validation,
    rate_limiting_trigger,
    network,
    conductor_handle
))]
pub async fn rate_limiting_workflow(
    workspace: Arc<RateLimitingWorkspace>,
    space: Arc<Space>,
    trigger_app_validation: TriggerSender,
    rate_limiting_trigger: TriggerSender,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
) -> WorkflowResult<WorkComplete> {
    let complete = rate_limiting_workflow_inner(
        workspace,
        space,
        network,
        conductor_handle,
        rate_limiting_trigger,
    )
    .await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // trigger other workflows
    trigger_app_validation.trigger(&"rate_limit_workflow");

    Ok(complete)
}

async fn rate_limiting_workflow_inner(
    workspace: Arc<RateLimitWorkspace>,
    space: Arc<Space>,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
    rate_limiting_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    todo!()
}
