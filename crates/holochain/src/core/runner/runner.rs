use crate::conductor::api::{CellConductorApi, CellConductorApiT};
use super::error::WorkflowRunResult;
use crate::core::{
    ribosome::WasmRibosome,
    state::workspace::{self, Workspace},
    workflow,
};
use futures::future::{BoxFuture, FutureExt};
use sx_state::{
    env::{Environment, WriteManager},
    prelude::*,
};
use workflow::{WorkflowCall, WorkflowEffects, WorkflowTrigger};
use workspace::WorkspaceError;

pub trait RunnerCellT<Api: CellConductorApiT = CellConductorApi>: Send + Sync {
    fn state_env(&self) -> Environment;
    fn get_ribosome(&self) -> WasmRibosome;
    fn get_conductor_api(&self) -> Api;
}

pub struct WorkflowRunner<'c, Cell: RunnerCellT>(&'c Cell);

impl<'c, Cell: RunnerCellT> WorkflowRunner<'c, Cell> {
    pub async fn run_workflow(&self, call: WorkflowCall) -> WorkflowRunResult<()> {
        let environ = self.0.state_env();
        let dbs = environ.dbs().await?;
        let env = environ.guard().await;

        // TODO: is it possible to DRY this up with a macro?
        match call {
            WorkflowCall::InvokeZome(invocation) => {
                let workspace = workspace::InvokeZomeWorkspace::new(env.reader()?, &dbs)?;
                let result =
                    workflow::invoke_zome(workspace, self.0.get_ribosome(), invocation).await?;
                self.finish_workflow(result).await?;
            }
            WorkflowCall::Genesis(dna, agent_id) => {
                let workspace = workspace::GenesisWorkspace::new(env.reader()?, &dbs)?;
                let api = self.0.get_conductor_api();
                let result = workflow::genesis(workspace, api, *dna, agent_id).await?;
                self.finish_workflow(result).await?;
            }
        }
        Ok(())
    }

    fn finish_workflow<'a, W: 'a + Workspace>(
        &'a self,
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
            {
                let writer = env.writer().map_err(Into::<WorkspaceError>::into)?;
                workspace.commit_txn(writer).map_err(Into::<WorkspaceError>::into)?;
            }
            for WorkflowTrigger { call, interval } in triggers {
                if let Some(_delay) = interval {
                    // FIXME: implement or discard
                    unimplemented!()
                } else {
                    self.run_workflow(call).await?
                }
            }
            for _callback in callbacks {
                // TODO
            }
            for _signal in signals {
                // TODO
            }

            Ok(())
        }
        .boxed()
    }
}
