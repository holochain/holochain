use crate::agent::SourceChain;
use crate::{cell::CellId, types::ZomeInvocation};
use holochain_persistence_api::cas::content::Address;

pub struct CascadingCursor;

pub fn get_cascading_cursor(_cid: &CellId) -> CascadingCursor {
    unimplemented!()
}

pub fn call_zome_function(_as_at: &Address, _args: &ZomeInvocation, _cursor: &mut CascadingCursor) {
    unimplemented!()
}

pub fn initialize_source_chain(cell_id: &CellId) -> SourceChain {
    unimplemented!()
}
