use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::entry_def::EntryVisibility;

holochain_wasmer_guest::holochain_externs!();

/// an example inner value that can be serialized into the contents of Entry::App()
#[derive(Deserialize, Serialize, SerializedBytes)]
enum ThisWasmEntry {
    AlwaysValidates,
    NeverValidates,
}

impl From<&ThisWasmEntry> for EntryDefId {
    fn from(entry: &ThisWasmEntry) -> Self {
        match entry {
            ThisWasmEntry::AlwaysValidates => "always_validates",
            ThisWasmEntry::NeverValidates => "never_validates",
        }.into()
    }
}

impl From<&ThisWasmEntry> for CrdtType {
    fn from(_: &ThisWasmEntry) -> Self {
        Self
    }
}

impl From<&ThisWasmEntry> for RequiredValidations {
    fn from(_: &ThisWasmEntry) -> Self {
        5.into()
    }
}

impl From<&ThisWasmEntry> for EntryVisibility {
    fn from(_: &ThisWasmEntry) -> Self {
        Self::Public
    }
}

impl From<&ThisWasmEntry> for EntryDef {
    fn from(entry: &ThisWasmEntry) -> Self {
        Self {
            id: entry.into(),
            crdt_type: entry.into(),
            required_validations: entry.into(),
            visibility: entry.into(),
        }
    }
}

#[no_mangle]
pub extern "C" fn entry_defs(_: GuestPtr) -> GuestPtr {
    let defs: EntryDefs = vec![
        (&ThisWasmEntry::AlwaysValidates).into(),
        (&ThisWasmEntry::NeverValidates).into(),
    ].into();

    ret!(GuestOutput::new(try_result!(EntryDefsCallbackResult::Defs(
        defs,
    ).try_into(), "failed to serialize entry defs return value")));
}

#[no_mangle]
pub extern "C" fn validate(host_allocation_ptr: GuestPtr) -> GuestPtr {
    // load host args
    let input: HostInput = host_args!(host_allocation_ptr);

    // extract the entry to validate
    let result: ValidateCallbackResult = match Entry::try_from(input.into_inner()) {
        // we do want to validate our app entries
        Ok(Entry::App(serialized_bytes)) => match ThisWasmEntry::try_from(serialized_bytes) {
            // the AlwaysValidates variant passes
            Ok(ThisWasmEntry::AlwaysValidates) => ValidateCallbackResult::Valid,
            // the NeverValidates variants fails
            Ok(ThisWasmEntry::NeverValidates) => ValidateCallbackResult::Invalid("NeverValidates never validates".to_string()),
            _ => ValidateCallbackResult::Invalid("Couldn't get ThisWasmEntry from the app entry".to_string()),
        },
        // other entry types we don't care about
        Ok(_) => ValidateCallbackResult::Valid,
        _ => ValidateCallbackResult::Invalid("Couldn't get App serialized bytes from host input".to_string()),
    };

    ret!(GuestOutput::new(try_result!(result.try_into(), "failed to serialize return value".to_string())));
}

/// we can write normal rust code with Results outside our externs
fn _commit_validate(to_commit: ThisWasmEntry) -> Result<GuestOutput, String> {
    let commit_output: CommitEntryOutput = host_call!(__commit_entry, CommitEntryInput::new(((&to_commit).into(), Entry::App(to_commit.try_into()?))))?;
    Ok(GuestOutput::new(commit_output.try_into()?))
}

#[no_mangle]
pub extern "C" fn always_validates(_: GuestPtr) -> GuestPtr {
    ret!(try_result!(_commit_validate(ThisWasmEntry::AlwaysValidates), "error processing commit"))
}
#[no_mangle]
pub extern "C" fn never_validates(_: GuestPtr) -> GuestPtr {
    ret!(try_result!(_commit_validate(ThisWasmEntry::NeverValidates), "error processing commit"))
}
