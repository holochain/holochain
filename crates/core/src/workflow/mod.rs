mod genesis;
mod invoke_zome;

pub(crate) use genesis::genesis;
pub(crate) use invoke_zome::invoke_zome;

use crate::state::workspace::Workspace;
use std::time::Duration;

use sx_types::{agent::AgentId, dna::Dna, nucleus::ZomeInvocation};
use thiserror::Error;

/// Specify the workflow-specific arguments to the functions that make the workflow go
/// It's intended that resources like Workspaces and Conductor APIs don't go here.
#[derive(Clone, Debug)]
pub enum WorkflowCall {
    InvokeZome(ZomeInvocation),
    Genesis(Box<Dna>, AgentId),
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
    pub callbacks: Vec<()>,
    pub signals: Vec<()>,
}

#[derive(Clone, Debug)]
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

// TODO: flesh out for real
#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("AgentId is invalid")]
    AgentIdInvalid(#[from] AgentId)
}

/// The `Result::Ok` of any workflow function is a `WorkflowEffects` struct.
pub type WorkflowResult<W> = Result<WorkflowEffects<W>, WorkflowError>;
