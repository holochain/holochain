use hdk3::prelude::*;

#[hdk_extern]
fn emit(_: ()) -> ExternResult<()> {
    emit_signal(&())?;
    Ok(())
}
