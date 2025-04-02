use hdk::prelude::*;

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    do_stuff()?;

    Ok(InitCallbackResult::Pass)
}

fn do_stuff() -> ExternResult<()> {
    let z = dna_info()?.hash;

    Ok(())
}

#[hdk_extern]
fn other_fn() -> ExternResult<()> {
    Ok(())
}
