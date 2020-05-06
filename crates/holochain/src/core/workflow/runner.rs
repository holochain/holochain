use crate::{
    conductor::Cell,
    core::{
        state::workspace::{self, Workspace, WorkspaceError},
        workflow::{self, WorkflowCall, WorkflowEffects, WorkflowTrigger},
    },
};
use futures::future::{join_all, BoxFuture, FutureExt};
use holochain_state::{env::WriteManager, prelude::*};
use workflow::{WorkflowCallback, WorkflowSignal};

use error::{WorkflowRunError, WorkflowRunResult};

pub mod error;

/// Functionality for running a Workflow for a Cell.
///
/// The Cell is put into an Arc because it is possible for a Workflow to spawn
/// additional workflows, which may run on separate threads, hence the Cell
/// reference needs to be long-lived and threadsafe.
///
/// FIXME: see finish_triggers for note on task spawning
pub struct WorkflowRunner<'b>(&'b Cell);

impl<'b> WorkflowRunner<'b> {
    pub fn new(cell: &'b Cell) -> Self {
        WorkflowRunner(cell)
    }

    pub async fn run_workflow(&self, call: WorkflowCall) -> WorkflowRunResult<()> {
        let environ = &self.0.state_env();
        let env = environ.guard().await;
        let dbs = environ.dbs().await?;
        let reader = env.reader()?;

        // TODO: is it possible to DRY this up with a macro?
        match call {
            WorkflowCall::InvokeZome(invocation) => {
                let workspace = workspace::InvokeZomeWorkspace::new(&reader, &dbs)
                    .map_err(|e| WorkflowRunError::from(e))?;
                let effects = workflow::invoke_zome::invoke_zome(
                    workspace,
                    self.0.get_ribosome().await?,
                    *invocation,
                )
                .await
                .map_err(|e| WorkflowRunError::from(e))?;
                self.finish(effects).await?;
            }
            WorkflowCall::Genesis(dna, agent_id) => {
                let workspace = workspace::GenesisWorkspace::new(&reader, &dbs)
                    .map_err(|e| WorkflowRunError::from(e))?;
                let api = self.0.get_conductor_api();
                let effects = workflow::genesis(workspace, api, *dna, agent_id).await?;
                self.finish(effects).await?;
            }
            WorkflowCall::InitializeZome => {
                todo!("Make initialize zome workflow");
            }
        }
        Ok(())
    }

    /// Apply the WorkflowEffects to finalize the Workflow.
    /// 1. Persist DB changes via `Workspace::commit_txn`
    /// 2. Call any Wasm callbacks
    /// 3. Emit any Signals
    /// 4. Trigger any subsequent Workflows
    fn finish<'a, W: 'a + Workspace>(
        &'a self,
        effects: WorkflowEffects<W>,
    ) -> BoxFuture<WorkflowRunResult<()>> {
        async move {
            let WorkflowEffects {
                workspace,
                triggers,
                callbacks,
                signals,
            } = effects;

            self.finish_workspace(workspace).await?;
            self.finish_callbacks(callbacks).await?;
            self.finish_signals(signals).await?;
            self.finish_triggers(triggers).await?;

            Ok(())
        }
        .boxed()
    }

    async fn finish_workspace<W: Workspace>(&self, workspace: W) -> WorkflowRunResult<()> {
        let arc = self.0.state_env();
        let env = arc.guard().await;
        let writer = env.writer().map_err(Into::<WorkspaceError>::into)?;
        workspace
            .commit_txn(writer)
            .map_err(Into::<WorkspaceError>::into)?;
        Ok(())
    }

    async fn finish_callbacks(&self, callbacks: Vec<WorkflowCallback>) -> WorkflowRunResult<()> {
        for _callback in callbacks {
            // TODO
        }
        Ok(())
    }

    async fn finish_signals(&self, signals: Vec<WorkflowSignal>) -> WorkflowRunResult<()> {
        for _signal in signals {
            // TODO
        }
        Ok(())
    }

    /// Spawn new tasks for each workflow trigger specified
    ///
    /// FIXME: this currently causes all workflow triggers to run on the same
    /// task as the causative workflow. If there is an explosion of workflow
    /// triggers, this will be a problem, and we will have to actually spawn
    /// a new task for each. The difficulty with that is that tokio::spawn
    /// requires the future to be 'static, which is currently not the case due
    /// to our LMDB Environment lifetimes.
    async fn finish_triggers(&self, triggers: Vec<WorkflowTrigger>) -> WorkflowRunResult<()> {
        let calls: Vec<_> = triggers
            .into_iter()
            .map(|WorkflowTrigger { call, interval }| {
                if let Some(_delay) = interval {
                    // FIXME: implement or discard
                    unimplemented!()
                } else {
                    self.run_workflow(call)
                }
            })
            .collect();
        join_all(calls).await.into_iter().collect()
    }
}
