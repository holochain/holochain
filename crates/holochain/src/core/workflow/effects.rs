use super::{error::WorkflowRunResult, run_workflow, Workflow};
use holochain_state::env::EnvironmentWrite;
use holochain_types::prelude::*;

/// A WorkflowEffects is returned from each Workspace function to declaratively
/// specify the side effects of the Workflow. It is taken by the `finish`
/// function to actually perform the side effects upon workflow completion.
// TODO: express in terms of two generic types instead of one associated type,
// which will allow us to remove the PhantomData
pub struct WorkflowEffects<'env, Wf: Workflow<'env>> {
    pub(super) workspace: Wf::Workspace,
    pub(super) callbacks: Vec<WorkflowCallback>,
    pub(super) signals: Vec<WorkflowSignal>,
    pub(super) triggers: Wf::Triggers,
    __lifetime: std::marker::PhantomData<&'env ()>,
}

impl<'env, Wf: Workflow<'env>> WorkflowEffects<'env, Wf> {
    /// Construct a WorkflowEffects.
    ///
    /// This is only necessary to hide away the `__lifetime` field.
    pub fn new(
        workspace: Wf::Workspace,
        callbacks: Vec<WorkflowCallback>,
        signals: Vec<WorkflowSignal>,
        triggers: Wf::Triggers,
    ) -> Self {
        Self {
            workspace,
            triggers,
            callbacks,
            signals,
            __lifetime: std::marker::PhantomData,
        }
    }
}

/// Specify a callback to execute in the DNA upon workflow completion
pub type WorkflowCallback = Todo;

/// Specify a Signal to be emitted upon workflow completion
pub type WorkflowSignal = Todo;

type TriggerOutput = tokio::task::JoinHandle<WorkflowRunResult<()>>;

/// Trait which defines additional workflows to be run after this one.
// TODO: B-01567: this can't be implemented as such until we find out how to
// dynamically create a Workspace via the trait-defined Workspace::new(),
// and to have the lifetimes match up.
// TODO: look into heterogeneous lists (frunk)

pub trait WorkflowTriggers<'env>: Send {
    /// Execute the triggers, causing other workflow tasks to be spawned
    fn run(self, env: EnvironmentWrite) -> TriggerOutput;

    /// FIXME: Placeholder
    fn is_empty(&self) -> bool {
        todo!("implement with hlist")
    }
}

impl<'env> WorkflowTriggers<'env> for () {
    fn run(self, _env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async { Ok(()) })
    }
}

impl<'env, W1> WorkflowTriggers<'env> for W1
where
    W1: 'static + Workflow<'env, Output = ()>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(env, self, todo!("get workspace"));
            Ok(())
        })
    }
}

impl<'env, T> WorkflowTriggers<'env> for Option<T>
where
    T: WorkflowTriggers<'env>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        if let Some(w) = self {
            w.run(env)
        } else {
            ().run(env)
        }
    }
}

impl<'env, W1, W2> WorkflowTriggers<'env> for (W1, W2)
where
    W1: 'static + Workflow<'env, Output = ()>,
    W2: 'static + Workflow<'env, Output = ()>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(env, self.0, todo!("get workspace"));
            let _handle = run_workflow(env, self.1, todo!("get workspace"));
            Ok(())
        })
    }
}
