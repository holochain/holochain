use hdi::map_extern::ExternResult;
use holochain_zome_types::clone::{ClonedCell, CreateCloneCellInput, DeleteCloneCellInput, DisableCloneCellInput, EnableCloneCellInput};
use crate::prelude::HDK;

// TODO docs
pub fn create_clone_cell(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().create_clone_cell(input))
}

// TODO docs
pub fn disable_clone_cell(input: DisableCloneCellInput) -> ExternResult<()> {
    HDK.with(|h| h.borrow().disable_clone_cell(input))
}

// TODO docs
pub fn enable_clone_cell(input: EnableCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().enable_clone_cell(input))
}

// TODO docs
pub fn delete_clone_cell(input: DeleteCloneCellInput) -> ExternResult<()> {
    HDK.with(|h| h.borrow().delete_clone_cell(input))
}

