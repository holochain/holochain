pub mod caller;
pub mod error;
mod genesis;
mod invoke_zome;

use crate::core::state::workspace::Workspace;
use caller::{run_workflow, WorkflowCaller};
use error::WorkflowRunResult;
use futures::{
    future::{BoxFuture, FutureExt},
    Future,
};
use holochain_state::env::EnvironmentRo;
use holochain_state::env::{EnvironmentRw, ReadManager};
use holochain_types::prelude::*;
use must_future::MustBoxFuture;
use std::time::Duration;
use thiserror::Error;

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<'env, WC: WorkflowCaller<'env>> {
    pub(super) workspace: WC::Workspace,
    pub(super) callbacks: Vec<WorkflowCallback>,
    pub(super) signals: Vec<WorkflowSignal>,
    pub(super) triggers: WC::Triggers,
    __lifetime: std::marker::PhantomData<&'env ()>,
}

impl<'env, WC: WorkflowCaller<'env>> WorkflowEffects<'env, WC> {
    pub fn new(
        workspace: WC::Workspace,
        callbacks: Vec<WorkflowCallback>,
        signals: Vec<WorkflowSignal>,
        triggers: WC::Triggers,
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

pub type WorkflowCallback = Todo;
pub type WorkflowSignal = Todo;

type TriggerOutput = tokio::task::JoinHandle<WorkflowRunResult<()>>;

/// Trait which defines additional workflows to be run after this one.
/// TODO: B-01567: this can't be implemented as such until we find out how to
/// dynamically create a Workspace via the trait-defined Workspace::new(),
/// and to have the lifetimes match up.
pub trait WorkflowTriggers<'env>: Send {
    fn run(self, env: EnvironmentRw) -> TriggerOutput;
}

impl<'env> WorkflowTriggers<'env> for () {
    fn run(self, _env: EnvironmentRw) -> TriggerOutput {
        tokio::spawn(async { Ok(()) })
    }
}

impl<'env, W1> WorkflowTriggers<'env> for W1
where
    W1: 'static + WorkflowCaller<'static, Output = ()>,
{
    fn run(self, env: EnvironmentRw) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(self, env, todo!("get workspace"));
            Ok(())
        })
    }
}

impl<'env, W1, W2> WorkflowTriggers<'env> for (W1, W2)
where
    W1: 'static + WorkflowCaller<'static, Output = ()>,
    W2: 'static + WorkflowCaller<'static, Output = ()>,
{
    fn run(self, env: EnvironmentRw) -> TriggerOutput {
        tokio::spawn(async {
            let _handle = run_workflow(self.0, env, todo!("get workspace"));
            let _handle = run_workflow(self.1, env, todo!("get workspace"));
            Ok(())
        })
    }
}
