mod genesis;
mod invoke_zome;

pub(crate) use genesis::genesis;
pub(crate) use invoke_zome::invoke_zome;

pub(crate) mod runner;

use crate::{
    cell::{Cell, CellId},
    conductor_api::ConductorCellApiT,
    nucleus::ZomeInvocation,
    state::workspace::{self, AppValidationWorkspace, InvokeZomeWorkspace, Workspace},
    workflow,
};
use std::time::Duration;
use sx_state::{
    db::DbManager,
    env::{Environment, WriteManager},
    error::DatabaseError,
    prelude::*,
};
use sx_types::{agent::AgentId, dna::Dna, shims::DhtOp};
use thiserror::Error;

#[derive(Clone, Debug)]
pub enum WorkflowCall {
    InvokeZome(ZomeInvocation),
    Genesis(Dna, AgentId),
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
    call: WorkflowCall,
    interval: Option<Duration>,
}

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
    #[error("It's too awful to tell!")]
    ItsAwful,
}

/// The `Result::Ok` of any workflow function is a `WorkflowEffects` struct.
pub type WorkflowResult<W> = Result<WorkflowEffects<W>, WorkflowError>;
