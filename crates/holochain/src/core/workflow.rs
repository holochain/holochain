mod effects;
pub mod error;
mod genesis;
mod invoke_zome;

pub use effects::*;

use crate::core::state::workspace::{Workspace, WorkspaceError};
use error::*;
use holochain_state::env::EnvironmentRw;
use holochain_state::env::WriteManager;
use must_future::MustBoxFuture;

pub trait Workflow<'env>: Sized + Send {
    type Output: Send;
    type Workspace: Workspace<'env> + 'env;
    type Triggers: WorkflowTriggers<'env>;

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;
}

pub async fn run_workflow<'env, O: Send, Wf: Workflow<'env, Output = O> + 'env>(
    wc: Wf,
    arc: EnvironmentRw,
    workspace: Wf::Workspace,
) -> WorkflowRunResult<O> {
    let (output, effects) = wc.workflow(workspace).await?;
    finish(arc, effects).await?;
    Ok(output)
}

/// Apply the WorkflowEffects to finalize the Workflow.
/// 1. Persist DB changes via `Workspace::commit_txn`
/// 2. Call any Wasm callbacks
/// 3. Emit any Signals
/// 4. Trigger any subsequent Workflows
async fn finish<'env, Wf: Workflow<'env>>(
    arc: EnvironmentRw,
    effects: WorkflowEffects<'env, Wf>,
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
        let writer = env
            .writer_unmanaged()
            .map_err(Into::<WorkspaceError>::into)?;
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
