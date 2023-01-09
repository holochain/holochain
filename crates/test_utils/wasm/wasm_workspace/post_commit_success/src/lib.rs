use hdk::prelude::*;

#[hdk_extern(infallible)]
fn post_commit(_: Vec<SignedActionHashed>) {
    // regression test: ensure that emit_signal works in post_commit
    emit_signal(&())?;
    Ok(())
}
