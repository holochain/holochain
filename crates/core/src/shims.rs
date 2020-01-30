use crate::error::SkunkResult;
use crate::agent::SourceChain;
use crate::types::ZomeInvocationResult;
use crate::{cell::CellId, types::ZomeInvocation};

use holochain_persistence_api::cas::content::Address as PersistenceAddress;

pub type Address = PersistenceAddress;

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct AgentId;


pub struct CapToken;
pub struct CapabilityRequest;
pub struct DhtTransform;
pub struct Dna;
pub struct Entry;
pub type JsonString = String;

pub struct Lib3hToClient;
pub struct Lib3hToClientResponse;
pub struct Lib3hClientProtocol;
pub struct Lib3hToServer;
pub struct Lib3hToServerResponse;
pub struct Lib3hServerProtocol;

pub enum ValidationResult {
    Valid,
    Invalid,
    Pending
}

/// Total hack just to have something to look at
pub struct Ribosome;
impl Ribosome {
    pub fn new(dna: Dna, cursor: CascadingCursor) -> Self {
        Self
    }

    pub fn run_validation(self, entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub fn call_zome_function(self, invocation: ZomeInvocation, source_chain: SourceChain) -> SkunkResult<(ZomeInvocationResult, CascadingCursor)> {
        unimplemented!()
    }
}

pub fn get_cascading_cursor(_cid: &CellId) -> CascadingCursor {
    unimplemented!()
}

pub fn call_zome_function(_as_at: &Address, _args: &ZomeInvocation, _cursor: &mut CascadingCursor) {
    unimplemented!()
}

pub fn initialize_source_chain(cell_id: &CellId) -> SourceChain {
    unimplemented!()
}
