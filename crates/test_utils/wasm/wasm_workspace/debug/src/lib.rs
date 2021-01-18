use hdk3::prelude::*;

#[hdk_extern]
fn debug(_: ()) -> ExternResult<()> {
    // hdk3::debug!("debug line numbers {}", "work");
    // hdk3::debug!("debug again");
    Ok(())
}
