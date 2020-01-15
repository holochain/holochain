use skunkworx_core_types::error::SkunkResult;
use crate::agent::SourceChain;
use crate::{cell::CellId, types::ZomeInvocation};
use holochain_persistence_api::cas::content::Address;

pub struct CascadingCursor;
pub struct CapToken;
pub struct Lib3hClientProtocol;
pub struct Lib3hServerProtocol;
pub struct DhtTransform;
pub struct Dna;

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

    pub fn run_validation(self, DhtTransform) -> ValidationResult {
        unimplemented!()
    }

    pub fn call_zome_function(self, invocation: ZomeInvocation, source_chain: SourceChain) -> SkunkResult<ZomeInvocationResult> {
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
