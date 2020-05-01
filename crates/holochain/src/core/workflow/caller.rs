use super::{
    error::{WorkflowResult, WorkflowRunResult},
    WorkflowEffects, WorkflowTriggers,
};
use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use futures::{Future, future::{BoxFuture, FutureExt}};
use holochain_state::env::WriteManager;
use holochain_state::env::{Environment, ReadManager};
use must_future::MustBoxFuture;
use std::pin::Pin;

// #[async_trait::async_trait]
pub trait WorkflowCaller<'env>: Sized + Send{
    type Output: Send;
    type Workspace: Workspace<'env>;
    type Triggers: WorkflowTriggers;

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;
}

// works
pub fn run_workflow<'env, WC: WorkflowCaller<'env> + 'env>(
    wc: WC,
    workspace: WC::Workspace,
    arc: Environment,
) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
    Box::new(async move {
        // unimplemented!()
        let (output, effects) = wc.workflow(workspace).await?;
        finish(arc, effects).await?;
        Ok(output)
    })
    // .boxed().into()
}

// works
pub fn run_workflow_2<'env, WC: WorkflowCaller<'env> + 'env>(
    wc: WC,
    workspace: WC::Workspace,
    arc: Environment,
) -> Pin<Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env>> {
    Box::pin(async move {
        // unimplemented!()
        let (output, effects) = wc.workflow(workspace).await?;
        finish(arc, effects).await?;
        Ok(output)
    })
    // .boxed().into()
}

pub fn run_workflow_3<'env, O: Send, WC: WorkflowCaller<'env, Output=O> + 'env>(
    wc: WC,
    workspace: WC::Workspace,
    arc: Environment,
// ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
) -> impl Future<Output = WorkflowRunResult<O>> + 'env {
    Box::pin(async move {
        // unimplemented!()
        let (output, effects) = wc.workflow(workspace).await?;
        finish(arc, effects).await?;
        Ok(output)
    })
    // .boxed().into()
}



/// Apply the WorkflowEffects to finalize the Workflow.
/// 1. Persist DB changes via `Workspace::commit_txn`
/// 2. Call any Wasm callbacks
/// 3. Emit any Signals
/// 4. Trigger any subsequent Workflows
async fn finish<'env, WC: WorkflowCaller<'env>>(
    arc: Environment,
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
        // let arc = cell.state_env();
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
