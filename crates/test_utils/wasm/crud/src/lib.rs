use hdk3::prelude::*;

const COUNTER_ID: &str = "counter";
#[derive(Default, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Counter(u32);

entry_defs!(
    def Counter EntryDef {
        id: COUNTER_ID.into(),
        visibility: EntryVisibility::Public,
        crdt_type: CrdtType,
        required_validations: RequiredValidations::default(),
    };
);

map_extern!(create, _create);
map_extern!(read, _read);
map_extern!(update, _update);
map_extern!(delete, _delete);

fn _create(_: ()) -> Result<HeaderHash, WasmError> {
    Ok(commit_entry!(&Counter::default())?)
}

fn _read(entry_hash: EntryHash) -> Result<GetEntryOutput, WasmError> {
    Ok(GetEntryOutput::new(get_entry!(entry_hash)?))
}

fn _update(_: ()) -> Result<(), WasmError> {
    // @todo
    Ok(())
}

fn _delete(header_hash: HeaderHash) -> Result<HeaderHash, WasmError> {
    Ok(remove_entry!(header_hash)?)
}
