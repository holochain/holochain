use element::ElementEntry;
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
    type Error = EntryError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        match entry {
            Entry::App(eb) => Ok(Self::try_from(SerializedBytes::from(eb.to_owned()))?),
            _ => Err(
                SerializedBytesError::FromBytes("failed to deserialize ThisWasmEntry".into())
                    .into(),
            ),
        }
    }
}

entry_defs![
    (&ThisWasmEntry::AlwaysValidates).into(),
    (&ThisWasmEntry::NeverValidates).into()
];

#[hdk_extern]
fn validate(element: Element) -> ExternResult<ValidateCallbackResult> {
    let entry = element.into_inner().1;
    let entry = match entry {
        ElementEntry::Present(e) => e,
        _ => return Ok(ValidateCallbackResult::Valid),
    };
    if let Entry::Agent(_) = entry {
        return Ok(ValidateCallbackResult::Valid);
    }
    Ok(match ThisWasmEntry::try_from(&entry) {
        Ok(ThisWasmEntry::AlwaysValidates) => ValidateCallbackResult::Valid,
        Ok(ThisWasmEntry::NeverValidates) => {
            ValidateCallbackResult::Invalid("NeverValidates never validates".to_string())
        }
        _ => ValidateCallbackResult::Invalid("Not a ThisWasmEntry".to_string()),
    })
}

fn _commit_validate(to_commit: ThisWasmEntry) -> ExternResult<HeaderHash> {
    Ok(create_entry!(&to_commit)?)
}

#[hdk_extern]
fn always_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::AlwaysValidates)
}

#[hdk_extern]
fn never_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::NeverValidates)
}
