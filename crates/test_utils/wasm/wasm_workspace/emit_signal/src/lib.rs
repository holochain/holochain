use hdk::prelude::*;
use integrity::{EntryTypes, TestEntry};
mod integrity;

#[hdk_extern]
fn emit(_: ()) -> ExternResult<()> {
    emit_signal(&())?;
    Ok(())
}

#[hdk_extern]
fn signal_others(signal: RemoteSignal) -> ExternResult<()> {
    send_remote_signal(&signal.signal, signal.agents)
}

#[hdk_extern]
fn recv_remote_signal(signal: ExternIO) -> ExternResult<()> {
    HDK.with(|h| h.borrow().emit_signal(AppSignal::new(signal)))
}

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    let mut fns = BTreeSet::new();
    fns.insert((zome_info()?.name, "recv_remote_signal".into()));
    let functions = GrantedFunctions::Listed(fns);
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Signal {
    Tested,
}

#[hdk_extern]
fn commit_entry_and_emit_signal_post_commit(_: ()) -> ExternResult<()> {
    create_entry(EntryTypes::TestEntry(TestEntry("test".to_string())))?;
    Ok(())
}

#[hdk_extern]
fn post_commit(_: Vec<SignedActionHashed>) -> ExternResult<()> {
    emit_signal(Signal::Tested)
}
