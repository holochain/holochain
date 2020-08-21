use hdk3::prelude::*;

#[hdk_extern]
fn debug(_: ()) -> ExternResult<()> {
    Ok(debug!("debug line numbers {}", "work")?)
}
