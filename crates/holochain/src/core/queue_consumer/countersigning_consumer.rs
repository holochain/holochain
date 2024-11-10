//! The queue consumer for the countersigning workflow.

use super::*;
#[cfg(feature = "unstable-countersigning")]
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tracing::*;

/// Spawn the QueueConsumer for the countersigning workflow
#[instrument(skip_all)]
pub(crate) fn spawn_countersigning_consumer(
    _space: Space,
    _workspace: Arc<CountersigningWorkspace>,
    cell_id: CellId,
    conductor: ConductorHandle,
    _integration_trigger: TriggerSender,
    _publish_trigger: TriggerSender,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    let _self_trigger = tx.clone();
    queue_consumer_cell_bound(
        "countersigning_consumer",
        cell_id.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        #[cfg(not(feature = "unstable-countersigning"))]
        move || async { WorkflowResult::Ok(WorkComplete::Complete) },
        #[cfg(feature = "unstable-countersigning")]
        move || {
            countersigning_workflow_fn(
                _space.clone(),
                _workspace.clone(),
                cell_id.clone(),
                conductor.clone(),
                _self_trigger.clone(),
                _integration_trigger.clone(),
                _publish_trigger.clone(),
            )
        },
    );

    tx
}

#[cfg(feature = "unstable-countersigning")]
async fn countersigning_workflow_fn(
    space: Space,
    workspace: Arc<CountersigningWorkspace>,
    cell_id: CellId,
    conductor: ConductorHandle,
    self_trigger: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let signal_tx = conductor
        .get_signal_tx(&cell_id)
        .await
        .map_err(WorkflowError::other)?;

    let cell = conductor
        .cell_by_id(&cell_id)
        .await
        .map_err(WorkflowError::other)?;
    let cell_network = cell.holochain_p2p_dna();

    let keystore = conductor.keystore().clone();

    countersigning_workflow(
        space.clone(),
        workspace,
        Arc::new(cell_network.clone()),
        keystore,
        cell_id.clone(),
        signal_tx,
        self_trigger.clone(),
        integration_trigger.clone(),
        publish_trigger.clone(),
    )
    .await
}
