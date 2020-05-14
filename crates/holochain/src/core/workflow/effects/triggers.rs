//! Workflow trigger types
//!
//! Workflow triggers can be specified by a Workflow -- that is, other Workflows
//! to run after the current workflow has completed. Since each Workflow has its
//! own type, we need a trait to specify a heterogenous collection of Workflows.
//! It's a little unwieldy, but it'll do for now.

use crate::core::workflow::{error::WorkflowRunResult, run_workflow, Workflow};
use either::Either;
use holochain_state::env::EnvironmentWrite;

type TriggerOutput = tokio::task::JoinHandle<WorkflowRunResult<()>>;

/// Trait which defines additional workflows to be run after this one.
// TODO: B-01567: this can't be completely implemented as such until we find out how to
// dynamically create a Workspace via the trait-defined Workspace::new(),
// and to have the lifetimes match up.
// TODO: look into heterogeneous lists (frunk)
pub trait WorkflowTriggers<'env>: Send {
    /// Execute the triggers, causing other workflow tasks to be spawned
    fn run(self, env: EnvironmentWrite) -> TriggerOutput;

    /// Specify if this value is "empty" or not
    fn is_empty(&self) -> bool;
}

/// The noop trigger
impl<'env> WorkflowTriggers<'env> for () {
    fn run(self, _env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async { Ok(()) })
    }

    fn is_empty(&self) -> bool {
        true
    }
}

/// Any trigger can be optional
impl<'env, T> WorkflowTriggers<'env> for Option<T>
where
    T: WorkflowTriggers<'env>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        if let Some(w) = self {
            w.run(env)
        } else {
            // noop trigger
            ().run(env)
        }
    }

    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

/// Either one trigger or another is still a trigger
impl<'env, T1, T2> WorkflowTriggers<'env> for Either<T1, T2>
where
    T1: WorkflowTriggers<'env>,
    T2: WorkflowTriggers<'env>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        match self {
            Either::Left(t) => t.run(env),
            Either::Right(t) => t.run(env),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Either::Left(t) => t.is_empty(),
            Either::Right(t) => t.is_empty(),
        }
    }
}

//-------------- BEGIN HETEROGENOUS LIST BOILERPLATE ----------------------
// Context:
// We want any number of `Workflow` structs to be considered a valid set of
// WorkflowTriggers. We can't use a simple Vec because Workflows have
// different types by design. Ergonomic options include:
//
// - using a heterogeneous list type like `frunk`, or
// - creating a `Triggerable` trait with no generics, which can be used in
//   a `Vec<Box<dyn Triggerable>>`
//
// For now, the most straightforward option is to manually implement a faux
// list type with tuples. The consequence is that each tuple arity needs an
// explicit definition, and if you run out of arity, you need to add an
// implementation here. The boilerplate is simple.
//
// Arity up to 3 is currently provided.

impl<'env, W0> WorkflowTriggers<'env> for W0
where
    W0: 'static + Workflow<'env, Output = ()>,
{
    #[allow(unreachable_code)]
    fn run(self, _env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async {
            // FIXME: Uncomment when this works, it panics atm
            //let _handle = run_workflow(env, self, todo!("get workspace"));
            Ok(())
        })
    }

    fn is_empty(&self) -> bool {
        false
    }
}

impl<'env, W0, W1> WorkflowTriggers<'env> for (W0, W1)
where
    W0: 'static + Workflow<'env, Output = ()>,
    W1: 'static + Workflow<'env, Output = ()>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(env, self.0, todo!("get workspace"));
            let _handle = run_workflow(env, self.1, todo!("get workspace"));
            Ok(())
        })
    }

    fn is_empty(&self) -> bool {
        false
    }
}

impl<'env, W0, W1, W2> WorkflowTriggers<'env> for (W0, W1, W2)
where
    W0: 'static + Workflow<'env, Output = ()>,
    W1: 'static + Workflow<'env, Output = ()>,
    W2: 'static + Workflow<'env, Output = ()>,
{
    #[allow(unreachable_code)]
    fn run(self, env: EnvironmentWrite) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(env, self.0, todo!("get workspace"));
            let _handle = run_workflow(env, self.1, todo!("get workspace"));
            let _handle = run_workflow(env, self.2, todo!("get workspace"));
            Ok(())
        })
    }

    fn is_empty(&self) -> bool {
        false
    }
}
