use hdk::prelude::*;

#[hdk_extern]
fn create_clone(input: CreateCloneCellInput) -> ExternResult<ClonedCell> {
    create_clone_cell(input)
}
