use hdk3::prelude::*;

#[hdk(extern)]
fn debug(_: ()) -> ExternResult<()> {
    Ok(debug!("debug line numbers {}", "work")?)
}
