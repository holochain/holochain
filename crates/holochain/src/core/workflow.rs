pub mod caller;
pub mod error;
mod genesis;
mod invoke_zome;

use caller::WorkflowCaller;
use error::WorkflowRunResult;
use holochain_types::prelude::*;
use std::time::Duration;
use thiserror::Error;

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<'env, WC: WorkflowCaller<'env>> {
    pub workspace: WC::Workspace,
    pub triggers: WC::Triggers,
    pub callbacks: Vec<WorkflowCallback>,
    pub signals: Vec<WorkflowSignal>,
    __lifetime: std::marker::PhantomData<&'env ()>,
}

pub type WorkflowCallback = Todo;
pub type WorkflowSignal = Todo;

pub trait WorkflowTriggers: Send + Sync {}

// #[derive(Debug)]
// pub struct WorkflowTrigger<'env, W: Workspace<'env>> {
//     pub(crate) caller: WorkflowCaller<'env, Output=(), Workspace=W>,
//     pub(crate) interval: Option<Duration>,
// }

// #[allow(dead_code)]
// impl WorkflowTrigger {
//     pub fn immediate(call: WorkflowCall) -> Self {
//         Self {
//             call,
//             interval: None,
//         }
//     }

//     pub fn delayed(call: WorkflowCall, interval: Duration) -> Self {
//         Self {
//             call,
//             interval: Some(interval),
//         }
//     }
// }
