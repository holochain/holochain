use hdk::prelude::*;

#[hdk_extern]
fn smash(_: ()) -> ExternResult<()> {
    loop {}
    Ok(())
}