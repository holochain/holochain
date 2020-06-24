use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::globals::ZomeGlobals;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;

holochain_wasmer_guest::holochain_externs!();

const POST_ID: &str = "post";
const POST_VALIDATIONS: u8 = 8;
#[derive(SerializedBytes, Serialize, Deserialize)]
struct Post;

impl From<&Post> for EntryDefId {
    fn from(_: &Post) -> Self {
        POST_ID.into()
    }
}

impl From<&Post> for EntryVisibility {
    fn from(_: &Post) -> Self {
        Self::Public
    }
}

impl From<&Post> for CrdtType {
    fn from(_: &Post) -> Self {
        Self
    }
}

impl From<&Post> for RequiredValidations {
    fn from(_: &Post) -> Self {
        POST_VALIDATIONS.into()
    }
}

impl From<&Post> for EntryDef {
    fn from(post: &Post) -> Self {
        Self {
            id: post.into(),
            visibility: post.into(),
            crdt_type: post.into(),
            required_validations: post.into(),
        }
    }
}

impl TryFrom<&Post> for Entry {
    type Error = SerializedBytesError;
    fn try_from(post: &Post) -> Result<Self, Self::Error> {
        Ok(Entry::App(post.try_into()?))
    }
}

#[no_mangle]
pub extern "C" fn entry_defs(_: GuestPtr) -> GuestPtr {
    let globals: ZomeGlobals = try_result!(host_call!(__globals, ()), "failed to get globals");

    let defs: EntryDefs = vec![
        (&Post).into(),
    ].into();

    ret!(GuestOutput::new(try_result!(EntryDefsCallbackResult::Defs(
        globals.zome_name,
        defs,
    ).try_into(), "failed to serialize entry defs return value")));
}

fn _commit_entry() -> Result<holo_hash_core::HoloHashCore, WasmError> {
    Ok(host_call!(__commit_entry, CommitEntryInput::new(((&Post).into(), (&Post).try_into()?)))?)
}

#[no_mangle]
pub extern "C" fn commit_entry(_: GuestPtr) -> GuestPtr {
    ret!(
        GuestOutput::new(
            try_result!(
                try_result!(_commit_entry(), "failed to commit post").try_into(),
                "failed to serialize commit post return"
            )
        )
    );
}
