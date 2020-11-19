use hdk3::prelude::*;

#[hdk_extern]
fn debug(_: ()) -> ExternResult<()> {
    debug!("debug line numbers {}", "work");
    Ok(())
}
