use hdi::map_extern::ExternResult;
use holochain_zome_types::clone::{ClonedCell, CreateCloneCellInput, DeleteCloneCellInput, DisableCloneCellInput, EnableCloneCellInput};
use crate::prelude::HDK;

/// Create a new cell in the current app based on the DNA of an existing cell in this app.
///
/// # Returns
///
/// A struct with the created cell's clone id and cell id.
pub fn create_clone_cell(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().create_clone_cell(input))
}

/// Disable a clone cell in the current app.
pub fn disable_clone_cell(input: DisableCloneCellInput) -> ExternResult<()> {
    HDK.with(|h| h.borrow().disable_clone_cell(input))
}

/// Enable a disabled clone cell in the current app.
///
/// # Returns
///
/// A struct with the enabled cell's clone id and cell id.
pub fn enable_clone_cell(input: EnableCloneCellInput) -> ExternResult<ClonedCell> {
    HDK.with(|h| h.borrow().enable_clone_cell(input))
}

/// Delete a clone cell in the current app.
pub fn delete_clone_cell(input: DeleteCloneCellInput) -> ExternResult<()> {
    HDK.with(|h| h.borrow().delete_clone_cell(input))
}

