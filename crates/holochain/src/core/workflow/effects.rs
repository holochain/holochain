
mod triggers;
pub use triggers::*;
use holochain_types::prelude::Todo;

/// A WorkflowEffects is returned from each Workspace function to declaratively
/// specify the side effects of the Workflow. It is taken by the `finish`
/// function to actually perform the side effects upon workflow completion.
// TODO: express in terms of two generic types instead of one associated type,
// which will allow us to remove the PhantomData
pub struct WorkflowEffects<Ws, Tr> {
    pub(super) workspace: Ws,
    pub(super) callbacks: Vec<WorkflowCallback>,
    pub(super) signals: Vec<WorkflowSignal>,
    pub(super) triggers: Tr,
}

/// Specify a callback to execute in the DNA upon workflow completion
pub type WorkflowCallback = Todo;

/// Specify a Signal to be emitted upon workflow completion
pub type WorkflowSignal = Todo;
