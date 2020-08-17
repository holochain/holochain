use hdk3::prelude::*;

#[hdk(extern)]
fn debug(_: ()) -> ExternResult<()> {
    debug!("debug line numbers {}", "work")?;
}
