use hdk::prelude::*;

#[hdk_extern]
fn create_clone(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    create_clone_cell(input)
}

#[hdk_extern]
fn disable_clone(input: DisableCloneCellInput) -> ExternResult<()> {
    disable_clone_cell(input)
}

#[hdk_extern]
fn enable_clone(input: EnableCloneCellInput) -> ExternResult<ClonedCell> {
    enable_clone_cell(input)
}

#[hdk_extern]
fn delete_clone(input: DeleteCloneCellInput) -> ExternResult<()> {
    delete_clone_cell(input)
}
