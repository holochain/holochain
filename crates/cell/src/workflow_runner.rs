
    use crate::{
        cell::CellId,
        nucleus::ZomeInvocation,
        state::workspace::{AppValidationWorkspace, InvokeZomeWorkspace, Workspace},
        workflow,
    };
    use std::time::Duration;
    use sx_state::{db::DbManager, env::{Env, WriteManager, EnvArc}, prelude::*, error::WorkspaceError};
    use thiserror::Error;

/// TODO: flesh out for real
#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("It's too awful to tell!")]
    ItsAwful,
}

/// TODO: flesh out for real
#[derive(Error, Debug)]
pub enum WorkflowRunError {
    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError)
}

/// The `Result::Ok` of any workflow function is a `WorkflowEffects` struct.
pub type WorkflowResult<W: Workspace> = Result<WorkflowEffects<W>, WorkflowError>;

/// Internal type to handle running workflows
type WorkflowRunResult<T> = Result<T, WorkflowRunError>;

pub enum WorkflowParams {
    InvokeZome(ZomeInvocation),
    AppValidation(Vec<DhtOp>),
    // {
    //     invocation: ZomeInvocation,
    //     source_chain: SourceChain<'_>,
    //     ribosome: Ribo,
    //     conductor_api: Api,
    // }
}

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<W: Workspace> {
    pub workspace: W,
    pub triggers: Vec<WorkflowTrigger>,
    pub callbacks: Vec<()>,
    pub signals: Vec<()>,
}

pub struct WorkflowTrigger {
    params: WorkflowParams,
    interval: Option<Duration>,
}

impl WorkflowTrigger {
    pub fn immediate(params: WorkflowParams) -> Self {
        Self {
            params,
            interval: None,
        }
    }

    pub fn delayed(params: WorkflowParams, interval: Duration) -> Self {
        Self {
            params,
            interval: Some(interval),
        }
    }
}

pub struct DhtOp;

#[cfg(todo)]
mod todo {

    pub struct WorkflowRunner {
        env: Env<'env>,
        dbs: DbManager<'env>,
    }

    impl WorkflowRunner {

        pub fn new(env: Env<'env>, dbs: DbManager) -> Self {

        }

        pub async fn run_workflow(&self, params: WorkflowParams) -> WorkflowRunResult<()> {
            let env = self.0.env();
            let dbs = self.0.dbs()?;
            match self.params {
                WorkflowParams::InvokeZome(invocation) => {
                    let workspace = InvokeZomeWorkspace::new(env.reader()?, dbs)?;
                    let result = workflow::invoke_zome(workspace, invocation).await?;
                    self.finish(result)
                }
                WorkflowParams::AppValidation(ops) => {
                    self.finish(app_validation(AppValidationWorkspace::new(unimplemented!()), ops).await?)
                }
            }
        }

        fn finish<W: Workspace>(&self, effects: WorkflowEffects<W>) -> WorkflowRunResult<()> {
            let mut writer = self.write_manager.writer()?;
            effects.workspace.commit_txn(writer)?;
            for trigger in effects.triggers {
                if let Some(delay) = trigger.interval {
                    unimplemented!()
                } else {

                }
            }
            Ok(())
        }
    }


    // pub struct WorkflowRun<'env, WM: WriteManager> {
    //     params: WorkflowParams,
    //     write_manager: WM,
    // }

    // impl WorkflowRun {

    //     async fn run(self) -> WorkflowRunResult<()> {
    //         match self.params {
    //             WorkflowParams::InvokeZome(invocation) => self.finish(
    //                 invoke_zome(InvokeZomeWorkspace::new(unimplemented!()), invocation).await?,
    //             ),
    //             WorkflowParams::AppValidation(ops) => self.finish(
    //                 app_validation(AppValidationWorkspace::new(unimplemented!()), ops).await?,
    //             ),
    //         }
    //     }

    //     /// Take the

    // }
}
