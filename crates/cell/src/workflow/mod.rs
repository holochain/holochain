mod health_check;
mod invoke_zome;
mod network_handler;
mod publish;

pub(crate) use health_check::health_check;
pub(crate) use invoke_zome::invoke_zome;
pub(crate) use network_handler::handle_network_message;
pub(crate) use publish::publish;

mod new_idea {

    use crate::{cell::CellId, nucleus::ZomeInvocation};
    use std::time::Duration;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum WorkflowError {
        #[error("It's too awful to tell!")]
        ItsAwful,
    }

    pub type WorkflowResult<T> = Result<T, WorkflowError>;

    pub async fn invoke_zome(
        workspace: InvokeZomeWorkspace,
        invocation: ZomeInvocation,
    ) -> WorkflowResult<WorkflowEffects<InvokeZomeWorkspace>> {
        unimplemented!()
    }

    pub async fn app_validation(
        workspace: AppValidationWorkspace,
        ops: Vec<DhtOp>,
    ) -> WorkflowResult<WorkflowEffects<AppValidationWorkspace>> {
        unimplemented!()
    }

    pub struct WorkflowEffects<W: Workspace> {
        workspace: W,
        triggers: Vec<WorkflowTrigger>,
        callbacks: Vec<()>,
        signals: Vec<()>,
    }

    pub struct WorkflowTrigger {
        workflow: WorkflowRun,
        interval: Option<Duration>,
    }

    impl WorkflowTrigger {
        pub fn immediate(workflow: WorkflowRun) -> Self {
            Self {
                workflow,
                interval: None,
            }
        }

        pub fn delayed(workflow: WorkflowRun, interval: Duration) -> Self {
            Self {
                workflow,
                interval: Some(interval),
            }
        }
    }

    pub enum WorkflowRun {
        InvokeZome(ZomeInvocation),
        AppValidation(Vec<DhtOp>),
        // {
        //     invocation: ZomeInvocation,
        //     source_chain: SourceChain<'_>,
        //     ribosome: Ribo,
        //     conductor_api: Api,
        // }
    }

    pub struct DhtOp;

    // invoke_zome(myworkspace, invocation)
    // vs
    // WorkflowRun::InvokeZome(invocation).run()

    impl WorkflowRun {
        async fn run(self) -> WorkflowResult<()> {
            match self {
                WorkflowRun::InvokeZome(invocation) => {
                    Self::finish(invoke_zome(InvokeZomeWorkspace::new(unimplemented!()), invocation).await?)
                }
                WorkflowRun::AppValidation(ops) => {
                    Self::finish(app_validation(AppValidationWorkspace::new(unimplemented!()), ops).await?)
                }
            }
        }

        /// Take the
        fn finish<W: Workspace>(effects: WorkflowEffects<W>) -> WorkflowResult<()> {
            Self::commit_workspace(effects.workspace)?;
            for trigger in effects.triggers {
                if let Some(delay) = trigger.interval {
                    // trigger with delay
                } else {
                    // trigger immediately
                }
            }
            Ok(())
        }

        fn commit_workspace<W: Workspace>(workspace: W) -> WorkflowResult<()> {
            unimplemented!()
        }
    }

    pub trait Workspace {}
    pub struct InvokeZomeWorkspace;
    pub struct AppValidationWorkspace;

    impl InvokeZomeWorkspace {
        pub fn new(cell_id: CellId) -> Self { Self }
    }
    impl AppValidationWorkspace {
        pub fn new(cell_id: CellId) -> Self { Self }
    }

    impl Workspace for InvokeZomeWorkspace {}
    impl Workspace for AppValidationWorkspace {}
}
