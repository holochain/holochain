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
        ..Default::default()
    };
);

fn post() -> Post {
    Post("foo".into())
}

fn _commit_entry(_: ()) -> Result<holo_hash::HeaderHash, WasmError> {
    Ok(commit_entry!(post())?)
}

fn _get_entry(_: ()) -> Result<GetOutput, WasmError> {
    Ok(GetOutput::new(get!(entry_hash!(post())?)?))
}

map_extern!(commit_entry, _commit_entry);
map_extern!(get_entry, _get_entry);
