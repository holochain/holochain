use super::error::WorkflowRunResult;
use crate::{
    conductor::Cell,
    core::{
        state::workspace::{self, Workspace, WorkspaceError},
        workflow::{self, WorkflowCall, WorkflowEffects, WorkflowTrigger},
    },
};
use futures::future::{BoxFuture, FutureExt};
use sx_state::{env::WriteManager, prelude::*};
use workflow::{WorkflowCallback, WorkflowSignal};

pub struct WorkflowRunner<'c>(&'c Cell);

impl<'c> WorkflowRunner<'c> {
    pub async fn run_workflow(&self, call: WorkflowCall) -> WorkflowRunResult<()> {
        let environ = self.0.state_env();
        let dbs = environ.dbs().await?;
        let env = environ.guard().await;
        let reader = env.reader()?;

        // TODO: is it possible to DRY this up with a macro?
        match call {
            WorkflowCall::InvokeZome(invocation) => {
                let workspace = workspace::InvokeZomeWorkspace::new(&reader, &dbs)?;
                let result =
                    workflow::invoke_zome(workspace, self.0.get_ribosome(), invocation).await?;
                self.finish(result).await?;
            }
            WorkflowCall::Genesis(dna, agent_id) => {
                let workspace = workspace::GenesisWorkspace::new(&reader, &dbs)?;
                let api = self.0.get_conductor_api();
                let result = workflow::genesis(workspace, api, *dna, agent_id).await?;
                self.finish(result).await?;
            }
        }
        Ok(())
    }

    fn finish<W: 'c + Workspace>(
        &self,
        effects: WorkflowEffects<W>,
    ) -> BoxFuture<WorkflowRunResult<()>> {
        async move {
            let arc = self.0.state_env();
            let env = arc.guard().await;
            let WorkflowEffects {
                workspace,
                triggers,
                callbacks,
                signals,
            } = effects;

            self.finish_workspace(workspace).await?;
            self.finish_callbacks(callbacks).await?;
            self.finish_signals(signals).await?;

            for WorkflowTrigger { call, interval } in triggers {
                if let Some(_delay) = interval {
                    // FIXME: implement or discard
                    unimplemented!()
                } else {
                    self.run_workflow(call).await?
                }
            }

            Ok(())
        }
        .boxed()
    }

    async fn finish_workspace<W: 'c + Workspace>(&self, workspace: W) -> WorkflowRunResult<()> {
        let arc = self.0.state_env();
        let env = arc.guard().await;
        let writer = env.writer().map_err(Into::<WorkspaceError>::into)?;
        workspace
            .commit_txn(writer)
            .map_err(Into::<WorkspaceError>::into)?;
        Ok(())
    }

    async fn finish_callbacks<'a>(
        &'a self,
        callbacks: Vec<WorkflowCallback>,
    ) -> WorkflowRunResult<()> {
        for _callback in callbacks {
            // TODO
        }
        Ok(())
    }

    async fn finish_signals<'a>(&'a self, signals: Vec<WorkflowSignal>) -> WorkflowRunResult<()> {
        for _signal in signals {
            // TODO
        }
        Ok(())
    }
}
