use hdk::prelude::*;

/// an example inner value that can be serialized into the contents of Entry::App()
#[derive(Deserialize, Serialize, SerializedBytes, Debug)]
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
            required_validations: entry.into(),
            visibility: entry.into(),
            required_validation_type: Default::default(),
        }
    }
}

impl TryFrom<&Entry> for ThisWasmEntry {
    type Error = WasmError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        match entry {
            Entry::App(eb) => Ok(Self::try_from(SerializedBytes::from(eb.to_owned()))?),
            _ => Err(SerializedBytesError::Deserialize(
                "failed to deserialize ThisWasmEntry".into(),
            )
            .into()),
        }
    }
}

impl TryFrom<&ThisWasmEntry> for Entry {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Ok(Entry::App(
            match AppEntryBytes::try_from(SerializedBytes::try_from(this_wasm_entry)?) {
                Ok(app_entry_bytes) => app_entry_bytes,
                Err(entry_error) => match entry_error {
                    EntryError::SerializedBytes(serialized_bytes_error) => {
                        return Err(WasmError::Serialize(serialized_bytes_error))
                    }
                    EntryError::EntryTooLarge(_) => {
                        return Err(WasmError::Guest(entry_error.to_string()))
                    }
                },
            },
        ))
    }
}

impl TryFrom<&ThisWasmEntry> for CreateInput {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Ok(Self::new(
            EntryDefId::from(this_wasm_entry),
            Entry::try_from(this_wasm_entry)?,
            ChainTopOrdering::default(),
        ))
    }
}

entry_defs![
    (&ThisWasmEntry::AlwaysValidates).into(),
    (&ThisWasmEntry::NeverValidates).into()
];

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreEntry {
            header:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: header, ..
                        },
                    ..
                },
            entry,
        } => match header.app_entry_type() {
            Some(app_entry_type) => {
                let this_zome = zome_info()?;
                let matches_entry_def = this_zome
                    .matches_entry_def_id(app_entry_type, (&ThisWasmEntry::AlwaysValidates).into())
                    || this_zome.matches_entry_def_id(
                        app_entry_type,
                        (&ThisWasmEntry::NeverValidates).into(),
                    );
                if matches_entry_def {
                    let entry = ThisWasmEntry::try_from(&entry)?;
                    match entry {
                        ThisWasmEntry::AlwaysValidates => Ok(ValidateCallbackResult::Valid),
                        ThisWasmEntry::NeverValidates => Ok(ValidateCallbackResult::Invalid(
                            "NeverValidates never validates".to_string(),
                        )),
                    }
                } else {
                    Ok(ValidateCallbackResult::Invalid(format!(
                        "Not a ThisWasmEntry but a {:?}",
                        header.entry_type()
                    )))
                }
            }
            None => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

fn _commit_validate(to_commit: ThisWasmEntry) -> ExternResult<HeaderHash> {
    create((&to_commit).try_into()?)
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn always_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::AlwaysValidates)
}

#[hdk_extern]
fn never_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::NeverValidates)
}
