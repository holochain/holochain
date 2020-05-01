pub mod caller;
pub mod error;
mod genesis;
mod invoke_zome;
pub(crate) use genesis::genesis;
pub(crate) use invoke_zome::invoke_zome;

#[cfg(test)]
use super::state::source_chain::SourceChainError;

use crate::{
    conductor::{api::error::ConductorApiError, Cell},
    core::state::workspace::{Workspace, WorkspaceError},
};
use caller::WorkflowCaller;
use error::WorkflowRunResult;
use futures::future::{BoxFuture, FutureExt};
use holochain_state::env::WriteManager;
use holochain_state::{db::DbManager, error::DatabaseError, prelude::Reader};
use holochain_types::{dna::Dna, nucleus::ZomeInvocation, prelude::*};
use must_future::MustBoxFuture;
use std::time::Duration;
use thiserror::Error;

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<'env, WC: WorkflowCaller<'env>> {
    pub workspace: WC::Workspace,
    pub triggers: WC::Triggers,
    pub callbacks: Vec<WorkflowCallback>,
    pub signals: Vec<WorkflowSignal>,
    _lifetime: std::marker::PhantomData<&'env ()>,
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
