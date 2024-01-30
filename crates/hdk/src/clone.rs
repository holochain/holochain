use hdi::map_extern::ExternResult;
use holochain_zome_types::clone::{ClonedCell, CreateCloneCellInput, DisableCloneCellInput};
use crate::prelude::HDK;

// TODO docs
pub fn create_clone_cell(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().create_clone_cell(input))
}

pub fn disable_clone_cell(input: DisableCloneCellInput) -> ExternResult<()> {
    // HDK.with(|h| h.borrow().disable_clone_cell(input))
    unimplemented!()
}
