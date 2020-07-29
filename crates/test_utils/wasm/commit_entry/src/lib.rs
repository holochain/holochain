use holochain_wasmer_guest::*;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::*;
use hdk3::prelude::*;

holochain_wasmer_guest::holochain_externs!();

const POST_ID: &str = "post";
#[derive(Default, SerializedBytes, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
struct Post(String);

entry_defs!(
    def Post EntryDef {
        id: POST_ID.into(),
        visibility: EntryVisibility::Public,
        crdt_type: CrdtType,
        required_validations: RequiredValidations::default(),
    };
);

fn post() -> Post {
    Post("foo".to_string())
}

map_extern!(commit_entry, _commit_entry);
map_extern!(get_entry, _get_entry);

fn _commit_entry(_: ()) -> Result<HeaderHash, WasmError> {
    Ok(commit_entry!(post())?)
}

fn _get_entry(_: ()) -> Result<GetEntryOutput, WasmError> {
    Ok(GetEntryOutput::new(get_entry!(entry_hash!(post())?)?))
}
