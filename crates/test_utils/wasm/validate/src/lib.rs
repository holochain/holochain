use hdk3::prelude::*;

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
        }
        .into()
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

impl TryFrom<&Entry> for ThisWasmEntry {
    type Error = ();
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        match entry {
            Entry::App(serialized_bytes) => Self::try_from(serialized_bytes),
            _ => Err(()),
        }
    }
}

entry_defs![
    (&ThisWasmEntry::AlwaysValidates).into(),
    (&ThisWasmEntry::NeverValidates).into()
];

#[hdk(extern)]
fn validate(entry: Entry) -> ExternResult<ValidateCallbackResult> {
    match ThisWasmEntry::try_from(serialized_bytes) {
        Ok(ThisWasmEntry::AlwaysValidates) => ValidateCallbackResult::Valid,
        Ok(ThisWasmEntry::NeverValidates) => {
            ValidateCallbackResult::Invalid("NeverValidates never validates".to_string())
        }
        _ => ValidateCallbackResult::Invalid("Not a ThisWasmEntry".to_string()),
    }
}

#[hdk(extern)]
fn commit_validate(to_commit: ThisWasmEntry) -> ExternResult<CommitEntryOutput> {
    Ok(commit_entry!(to_commit)?)
}
