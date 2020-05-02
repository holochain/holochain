pub mod caller;
pub mod error;
mod genesis;
mod invoke_zome;

use caller::{run_workflow_5, WorkflowCaller};
use error::WorkflowRunResult;
use futures::{Future, future::{BoxFuture, FutureExt}};
use holochain_types::prelude::*;
use must_future::MustBoxFuture;
use std::time::Duration;
use thiserror::Error;
use holochain_state::env::Environment;
use holochain_state::env::ReadManager;
use crate::core::state::workspace::Workspace;

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

// pub type TriggerOutput = Box<dyn Future<Output=WorkflowRunResult<()>> + 'env>;
pub type TriggerOutput = tokio::task::JoinHandle<WorkflowRunResult<()>>;

pub trait WorkflowTriggers<'env>: Send {
    fn run(self, env: Environment) -> TriggerOutput;
}

impl<'env> WorkflowTriggers<'env> for () {
    fn run(self, env: Environment) -> TriggerOutput {
        tokio::spawn(async { Ok(()) })
    }
}

impl<'env, W1: 'static + WorkflowCaller<'static, Output=()>> WorkflowTriggers<'env> for W1 {
    fn run(self, env: Environment) -> TriggerOutput {
        // Box::new(
        tokio::spawn(async {
            let _handle = run_workflow_5(self, env);
            Ok(())
        })
    // )
    }    
}

// impl<'env, W1: WorkflowCaller<'env> + 'env> WorkflowTriggers for W1 {
//     fn run(self, env: Environment) -> Box<dyn Future<Output=WorkflowRunResult<W1::Output>> + 'env> {
//         Box::new(async {
//             let e = env.guard().await;
//             let reader = e.reader()?;
//             let dbs = env.dbs().await?;
//             let workspace = W1::Workspace::new(&reader, &dbs)?;
//             run_workflow_5(self, workspace, env).await
//         })
//     }    
// }