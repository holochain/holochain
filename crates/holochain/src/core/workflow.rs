pub mod caller;
pub mod error;
mod genesis;
mod invoke_zome;

use caller::WorkflowCaller;
use error::WorkflowRunResult;
use holochain_types::prelude::*;
use std::time::Duration;
use thiserror::Error;
use must_future::MustBoxFuture;

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

pub trait WorkflowTriggers: Send + Sync {
    // fn run(self) -> MustBoxFuture<'static, WorkflowRunResult<()>>;
}
