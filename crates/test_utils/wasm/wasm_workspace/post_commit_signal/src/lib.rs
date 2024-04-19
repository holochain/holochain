use hdk::prelude::*;
use integrity::{EntryTypes, TestEntry};
mod integrity;

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
