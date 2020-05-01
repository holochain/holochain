
use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use super::{WorkflowTriggers, error::{WorkflowResult, WorkflowRunResult}, WorkflowEffects};
use futures::future::{BoxFuture, FutureExt};
use holochain_state::env::WriteManager;
use holochain_state::{db::DbManager, error::DatabaseError, prelude::Reader};
use holochain_types::{dna::Dna, nucleus::ZomeInvocation, prelude::*};
use must_future::MustBoxFuture;
use std::time::Duration;
use thiserror::Error;


pub trait WorkflowCaller<'env>: Sized + Send + Sync {
    type Output;
    type Workspace: Workspace<'env>;
    type Triggers: WorkflowTriggers;

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>>;
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
