mod genesis;
mod invoke_zome;
pub mod runner;
pub(crate) use genesis::genesis;
pub(crate) use invoke_zome::invoke_zome;

#[cfg(test)]
use super::state::source_chain::SourceChainError;

use crate::{
    conductor::api::error::ConductorApiError,
    core::state::workspace::{Workspace, WorkspaceError},
};
use holochain_state::{db::DbManager, error::DatabaseError, prelude::Reader};
use holochain_types::{dna::Dna, nucleus::ZomeInvocation, prelude::*};
use must_future::MustBoxFuture;
use runner::error::WorkflowRunResult;
use std::time::Duration;
use thiserror::Error;

pub trait WorkflowCaller<'env> {
    type Output;
    type Workspace: Workspace<'env>;
    
    fn call(self) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self::Workspace>>;
}

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<'env, W: Workspace<'env>> {
    pub workspace: W,
    pub triggers: WorkflowTriggers,
    pub callbacks: Vec<WorkflowCallback>,
    pub signals: Vec<WorkflowSignal>,
    _lifetime: std::marker::PhantomData<&'env ()>,
}

pub type WorkflowCallback = Todo;
pub type WorkflowSignal = Todo;
pub type WorkflowTriggers = Todo;

// #[derive(Debug)]
// pub struct WorkflowTrigger<O, W: Workspace> {
//     pub(crate) call: WorkflowCaller<O, W>,
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

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentHash),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] ConductorApiError),

    #[error("Workspace error: {0}")]
    WorkspaceError(#[from] WorkspaceError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[cfg(test)]
    #[error("Source chain error: {0}")]
    SourceChainError(#[from] SourceChainError),
}

/// The `Result::Ok` of any workflow function is a `WorkflowEffects` struct.
pub type WorkflowResult<'env, O, W> = Result<(O, WorkflowEffects<'env, W>), WorkflowError>;
