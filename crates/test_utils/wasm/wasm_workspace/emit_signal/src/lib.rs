use hdk3::prelude::*;

#[hdk_extern]
fn emit(_: ()) -> ExternResult<()> {
    debug!("hm?");
    emit_signal!(());
    Ok(())
}
