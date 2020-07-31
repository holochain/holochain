use hdk3::prelude::*;

const VOTES_ID: &str = "votes";
#[derive(Default, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Votes(Uuid);

impl Votes {
    fn new() -> Result<Self, WasmError> {
        let bytes = random_bytes!(128)?;
        let uuid: Uuid = bytes.into();
        Self(uuid)
    }
}

entry_defs!(
    def Votes EntryDef {
        id: VOTES_ID.into(),
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
    Ok(commit_entry!(&Votes::new()?)?;
}

fn _read(header_hash: HeaderHash) -> Result<Element, WasmError> {
    Ok(get!(header_hash)?)
}

fn _tally(uuid: Uuid) -> Result<Option<u32>, WasmError> {
    let base = get!(Votes(uuid))?;

    // recursively get_updates! to build entire tree
    // count tree, add updates and subtract deletes
}

fn _increment(header_hash: HeaderHash) -> Result<HeaderHash, WasmError> {
    let element = get!(header_hash)?;
    let entry: Entry = element.into_inner()?.entry().as_option()?.clone();
    match entry {
        Entry::App(sb) => {
            match Votes::try_from(sb)? {
                Votes(uuid) => {
                    Ok(update_entry!(header_hash, &Votes(uuid))?)
                }
                _ => Err(..)?,
            }
        },
        None => Err(..)?,
    }
}

fn _decrement(header_hash: HeaderHash) -> Result<HeaderHash, WasmError> {
    Ok(remove_entry!(header_hash)?)
}

fn validate_delete(header_hash: HeaderHash) -> Result<ValidateCallbackResult, WasmError> {
    let element = get!(header_hash)?;
    let header = Header:try_into(element);

    let update_element = get!(header.revises_address)?;
    let update_header = Header::try_into(element);

    if update_header.author() == header.author() {
        ValidateCallbackResult::Valid
    } else {
        ValidateCallbackResult::Invalid
    }
}
