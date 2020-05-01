
use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use super::{WorkflowTriggers, error::{WorkflowResult, WorkflowRunResult}, WorkflowEffects};
use futures::future::{FutureExt};
use holochain_state::env::WriteManager;
use must_future::MustBoxFuture;
use holochain_state::env::ReadManager;


pub trait WorkflowCaller<'env>: Sized + Send + Sync {
    type Output: Send;
    type Workspace: Workspace<'env>;
    type Triggers: WorkflowTriggers;

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;

    fn run<W: WorkflowCaller<'env>>(w: W, cell: &'env Cell) -> MustBoxFuture<WorkflowRunResult<Self::Output>> {
        async {
            let arc = cell.state_env();
            let env = arc.guard().await;
            let dbs = arc.dbs().await?;
            let reader = env.reader()?;
            let workspace = W::Workspace::new(&reader, &dbs)?;
            let (output, effects) = w.workflow(workspace).await?;
            finish(cell, effects).await?;
            Ok(output)
        }
        .boxed().into()
    }
}

/// Apply the WorkflowEffects to finalize the Workflow.
/// 1. Persist DB changes via `Workspace::commit_txn`
/// 2. Call any Wasm callbacks
/// 3. Emit any Signals
/// 4. Trigger any subsequent Workflows
async fn finish<'env, WC: WorkflowCaller<'env>>(
    cell: &'env Cell,
    effects: WorkflowEffects<'env, WC>,
) -> WorkflowRunResult<()> {
    let WorkflowEffects {
        workspace,
        triggers,
        callbacks,
        signals,
        ..
    } = effects;

    {
        let arc = cell.state_env();
        let env = arc.guard().await;
        let writer = env.writer().map_err(Into::<WorkspaceError>::into)?;
        workspace
            .commit_txn(writer)
            .map_err(Into::<WorkspaceError>::into)?;
    }

    {
        for _callback in callbacks {
            // TODO
        }
    }

    {
        for _signal in signals {
            // TODO
        }
    }
    // self.finish_triggers(triggers).await?;

    Ok(())
}
