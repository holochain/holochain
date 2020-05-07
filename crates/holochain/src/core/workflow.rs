mod genesis;
mod invoke_zome;
pub mod runner;
pub(crate) use genesis::genesis;

use crate::{
    conductor::api::error::ConductorApiError,
    core::state::workspace::{Workspace, WorkspaceError},
};
use holochain_state::error::DatabaseError;
use holochain_types::{dna::DnaFile, nucleus::ZomeInvocation, prelude::*};
use std::time::Duration;
use thiserror::Error;

use super::{ribosome::error::RibosomeError, state::source_chain::SourceChainError};

#[cfg(test)]
pub use genesis::tests::fake_genesis;

/// Specify the workflow-specific arguments to the functions that make the workflow go
/// It's intended that resources like Workspaces and Conductor APIs don't go here.
#[derive(Debug)]
pub enum WorkflowCall {
    InvokeZome(Box<ZomeInvocation>),
    InitializeZome,
    Genesis(Box<DnaFile>, AgentPubKey),
    // AppValidation(Vec<DhtOp>),
    // {
    //     invocation: ZomeInvocation,
    //     source_chain: SourceChain<'_>,
    //     ribosome: Ribo,
    //     conductor_api: Api,
    // }
}

/// A WorkflowEffects is returned from each Workspace function.
/// It's just a data structure with no methods of its own, hence the public fields
pub struct WorkflowEffects<W: Workspace> {
    pub workspace: W,
    pub triggers: Vec<WorkflowTrigger>,
    pub callbacks: Vec<WorkflowCallback>,
    pub signals: Vec<WorkflowSignal>,
}

pub type WorkflowCallback = ();
pub type WorkflowSignal = ();

#[derive(Debug)]
pub struct WorkflowTrigger {
    pub(crate) call: WorkflowCall,
    pub(crate) interval: Option<Duration>,
}

#[allow(dead_code)]
impl WorkflowTrigger {
    pub fn immediate(call: WorkflowCall) -> Self {
        Self {
            call,
            interval: None,
        }
    }

    pub fn delayed(call: WorkflowCall, interval: Duration) -> Self {
        Self {
            call,
            interval: Some(interval),
        }
    }
}

impl<W: Workspace> std::fmt::Debug for WorkflowEffects<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowEffects")
            .field("triggers", &self.triggers)
            .field("callbacks", &self.callbacks)
            .field("signals", &self.signals)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentPubKey),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] Box<ConductorApiError>),

    #[error("Workspace error: {0}")]
    WorkspaceError(#[from] WorkspaceError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),

    #[error("Source chain error: {0}")]
    SourceChainError(#[from] SourceChainError),

    #[error("Capability token missing")]
    CapabilityMissing,
}

/// The `Result::Ok` of any workflow function is a `WorkflowEffects` struct.
pub type WorkflowResult<W> = Result<WorkflowEffects<W>, WorkflowError>;
