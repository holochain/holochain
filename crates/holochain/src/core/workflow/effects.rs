use super::Workflow;

mod triggers;
pub use triggers::*;
use holochain_types::prelude::Todo;

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
