use super::{
    error::{WorkflowResult, WorkflowRunResult},
    WorkflowEffects, WorkflowTriggers,
};
use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use futures::{
    future::{BoxFuture, FutureExt},
    Future,
};
use holochain_state::env::WriteManager;
use holochain_state::{
    env::{Environment, ReadManager, EnvironmentReadonly},
    prelude::Reader,
};
use must_future::MustBoxFuture;
use std::pin::Pin;

// #[async_trait::async_trait]
pub trait WorkflowCaller<'env>: Sized + Send {
    type Output: Send;
    type Workspace: Workspace<'env>;
    type Triggers: WorkflowTriggers<'env>;

    fn workflow(
        self,
        env: EnvironmentReadonly,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;
}

// // works
// pub fn run_workflow<'env, WC: WorkflowCaller<'env> + 'env>(
//     wc: WC,
//     workspace: WC::Workspace,
//     arc: Environment,
// ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
//     Box::new(async move {
//         // unimplemented!()
//         let (output, effects) = wc.workflow(workspace).await?;
//         finish(arc, effects).await?;
//         Ok(output)
//     })
//     // .boxed().into()
// }

// // works
// pub fn run_workflow_2<'env, WC: WorkflowCaller<'env> + 'env>(
//     wc: WC,
//     workspace: WC::Workspace,
//     arc: Environment,
// ) -> Pin<Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env>> {
//     Box::pin(async move {
//         // unimplemented!()
//         let (output, effects) = wc.workflow(workspace).await?;
//         finish(arc, effects).await?;
//         Ok(output)
//     })
//     // .boxed().into()
// }

// pub fn run_workflow_3<'env, O: Send, WC: WorkflowCaller<'env, Output = O> + 'env>(
//     wc: WC,
//     workspace: WC::Workspace,
//     arc: Environment,
//     // ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
// ) -> impl Future<Output = WorkflowRunResult<O>> + 'env {
//     async move {
//         // unimplemented!()
//         let (output, effects) = wc.workflow(workspace).await?;
//         finish(arc, effects).await?;
//         Ok(output)
//     }
//     // .boxed().into()
// }

// pub async fn run_workflow_4<'env, O: Send, WC: WorkflowCaller<'env, Output = O> + 'env>(
//     wc: WC,
//     workspace: WC::Workspace,
//     arc: Environment,
//     // ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
// ) -> WorkflowRunResult<O> {
//     // async move {
//     // unimplemented!()
//     let (output, effects) = wc.workflow(workspace).await?;
//     finish(arc, effects).await?;
//     Ok(output)
//     // }.await
//     // .boxed().into()
// }

pub async fn run_workflow_5<'env, O: Send, WC: WorkflowCaller<'env, Output = O> + 'env>(
    wc: WC,
    arc: Environment,
    // ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
) -> WorkflowRunResult<O> {
    let (output, effects) = wc.workflow(arc.clone().into()).await?;
    finish(arc, effects).await?;
    Ok(output)
}

// pub fn run_workflow_task<
//     'env,
//     O: Send + 'static,
//     WC: WorkflowCaller<'static, Output = O> + 'static,
// >(
//     wc: WC,
//     // workspace: WC::Workspace,
//     arc: Environment,
//     // ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
// ) -> tokio::task::JoinHandle<WorkflowRunResult<O>> {
//     let arc = arc.clone();
//     tokio::spawn(async move {
//         let arc = arc.clone();
//         let env = arc.guard_readonly().await;
//         let reader: Reader<'static> = env.reader()?;
//         let dbs = arc.dbs().await?;
//         let workspace = WC::Workspace::new(&reader, &dbs)?;
//         let (output, effects) = wc.workflow(workspace).await?;
//         finish(arc.clone(), effects).await?;
//         Ok(output)
//     })
// }

// FAILS
// pub async fn run_workflow_5<'env, WC: WorkflowCaller<'env> + 'env>(
//     wc: WC,
//     workspace: WC::Workspace,
//     arc: Environment,
// // ) -> Box<dyn Future<Output = WorkflowRunResult<WC::Output>> + 'env> {
// ) -> WorkflowRunResult<WC::Output> {
//     // async move {
//         // unimplemented!()
//         let (output, effects) = wc.workflow(workspace).await?;
//         finish(arc, effects).await?;
//         Ok(output)
//     // }.await
//     // .boxed().into()
// }

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
        triggers,
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
    let handle = triggers.run(arc);

    Ok(())
}
