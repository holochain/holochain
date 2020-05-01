use super::{
    error::{WorkflowResult, WorkflowRunResult},
    WorkflowEffects, WorkflowTriggers,
};
use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use futures::future::{BoxFuture, FutureExt};
use holochain_state::env::WriteManager;
use holochain_state::env::{Environment, ReadManager};
use must_future::MustBoxFuture;

// #[async_trait::async_trait]
pub trait WorkflowCaller<'env>: Sized + Send {
    type Output: Send;
    type Workspace: Workspace<'env>;
    type Triggers: WorkflowTriggers;

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;

    fn run(self, cell: &'env Cell) -> MustBoxFuture<'env, WorkflowRunResult<Self::Output>> {
        let arc = cell.state_env();
        async {
            let arc = arc;
            {
                let env = arc.guard().await;
                let dbs = arc.dbs().await?;
                let reader = env.reader()?;
                let workspace = Self::Workspace::new(&reader, &dbs)?;
                let (output, effects) = self.workflow(workspace).await?;
                finish(cell, effects).await?;
                Ok(output)
            }
        }
        .boxed()
        .into()
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
        // triggers,
        callbacks,
        signals,
        ..
    } = effects;

    // finish workspace
    {
        let arc = cell.state_env();
        let env = arc.guard().await;
        let writer = env.writer().map_err(Into::<WorkspaceError>::into)?;
        workspace
            .commit_txn(writer)
            .map_err(Into::<WorkspaceError>::into)?;
    }

    // finish callbacks
    {
        for _callback in callbacks {
            // TODO
        }
    }

    // finish signals
    {
        for _signal in signals {
            // TODO
        }
    }

    // finish triggers
    // triggers.run().await?;

    Ok(())
}
