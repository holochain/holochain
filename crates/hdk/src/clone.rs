use hdi::map_extern::ExternResult;
use holochain_zome_types::clone::{ClonedCell, CreateCloneCellInput};
use crate::prelude::HDK;

// TODO docs
pub fn create_clone_cell(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().create_clone_cell(input))
}
