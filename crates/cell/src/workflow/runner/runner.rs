use super::error::WorkflowRunResult;
use crate::{
    cell::{Cell, CellId},
    conductor_api::ConductorCellApiT,
    nucleus::ZomeInvocation,
    state::workspace::{self, AppValidationWorkspace, InvokeZomeWorkspace, Workspace},
    workflow,
};
use futures::future::{BoxFuture, FutureExt};
use std::time::Duration;
use sx_state::{
    db::DbManager,
    env::{Environment, WriteManager},
    error::DatabaseError,
    prelude::*,
};
use workflow::{WorkflowEffects, WorkflowCall, WorkflowTrigger};
use workspace::WorkspaceError;


impl<Api: ConductorCellApiT> Cell<Api> {
    pub async fn run_workflow(&self, call: WorkflowCall) -> WorkflowRunResult<()> {
        let env = self.state_env();
        let dbs = env.dbs()?;

        // TODO: is it possible to DRY this up with a macro?
        match call {
            WorkflowCall::InvokeZome(invocation) => {
                let workspace = workspace::InvokeZomeWorkspace::new(env.reader()?, &dbs)?;
                let result =
                    workflow::invoke_zome(workspace, self.get_ribosome(), invocation).await?;
                self.finish_workflow(result).await?;
            },
            WorkflowCall::Genesis(dna, agent_id) => {
                let workspace = workspace::GenesisWorkspace::new(env.reader()?, &dbs)?;
                let result =
                    workflow::genesis(workspace, dna, agent_id).await?;
                self.finish_workflow(result).await?;
            }
        }
        Ok(())
    }

    fn finish_workflow<W: Workspace>(
        &self,
        effects: WorkflowEffects<W>,
    ) -> BoxFuture<WorkflowRunResult<()>> {
        let env = self.state_env();
        let triggers = effects.triggers.clone();
        let result: Result<(), WorkspaceError> = env.writer().map_err(Into::<WorkspaceError>::into).and_then(|writer| {
            effects.workspace.commit_txn(writer).map_err(Into::into)
        });
        async move {
            for WorkflowTrigger { call, interval } in triggers {
                if let Some(delay) = interval {
                    unimplemented!()
                } else {
                    self.run_workflow(call).await?
                }
            }
            Ok(())
        }.boxed()
    }
}
