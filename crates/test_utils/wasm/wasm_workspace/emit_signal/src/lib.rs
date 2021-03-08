use hdk::prelude::*;

#[hdk_extern]
fn emit(_: ()) -> ExternResult<()> {
    emit_signal(&())?;
    Ok(())
}

#[hdk_extern]
fn signal_others(signal: RemoteSignal) -> ExternResult<()> {
    remote_signal(&signal.signal, signal.agents)
}

#[hdk_extern]
fn recv_remote_signal(signal: ExternIO) -> ExternResult<()> {
    HDK.read().emit_signal(AppSignal::new(signal))
}

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    let mut functions: GrantedFunctions = HashSet::new();
    functions.insert((zome_info()?.zome_name, "recv_remote_signal".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}
