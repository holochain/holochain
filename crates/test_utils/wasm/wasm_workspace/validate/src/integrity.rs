use holochain_deterministic_integrity::prelude::*;

/// an example inner value that can be serialized into the contents of Entry::App()
#[derive(Deserialize, Serialize, SerializedBytes, Debug, EntryDefRegistration)]
pub enum ThisWasmEntry {
    #[entry_def(required_validations = 5)]
    AlwaysValidates,
    #[entry_def(required_validations = 5)]
    NeverValidates,
}

impl TryFrom<&Entry> for ThisWasmEntry {
    type Error = WasmError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        match entry {
            Entry::App(eb) => Ok(Self::try_from(SerializedBytes::from(eb.to_owned()))
                .map_err(|e| wasm_error!(e.into()))?),
            _ => Err(wasm_error!(SerializedBytesError::Deserialize(
                "failed to deserialize ThisWasmEntry".into(),
            )
            .into())),
        }
    }
}

impl TryFrom<&ThisWasmEntry> for Entry {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Ok(Entry::App(AppEntryBytes::try_from(this_wasm_entry)?))
    }
}

impl TryFrom<&ThisWasmEntry> for AppEntryBytes {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        match AppEntryBytes::try_from(
            SerializedBytes::try_from(this_wasm_entry).map_err(|e| wasm_error!(e.into()))?,
        ) {
            Ok(app_entry_bytes) => Ok(app_entry_bytes),
            Err(entry_error) => match entry_error {
                EntryError::SerializedBytes(serialized_bytes_error) => Err(wasm_error!(
                    WasmErrorInner::Serialize(serialized_bytes_error)
                )),
                EntryError::EntryTooLarge(_) => {
                    Err(wasm_error!(WasmErrorInner::Guest(entry_error.to_string())))
                }
            },
        }
    }
}

impl From<&ThisWasmEntry> for LocalZomeTypeId {
    fn from(_: &ThisWasmEntry) -> Self {
        Self(0)
    }
}

impl TryFrom<&ThisWasmEntry> for EntryDefIndex {
    type Error = WasmError;

    fn try_from(value: &ThisWasmEntry) -> Result<Self, Self::Error> {
        zome_info()?
            .zome_types
            .entries
            .to_global_scope(value)
            .map(Self::from)
            .ok_or_else(|| {
                wasm_error!(WasmErrorInner::Guest(
                    "ThisWasmEntry did not map to an EntryDefIndex within this scope".to_string(),
                ))
            })
    }
}

#[hdk_extern]
pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
    Ok(EntryDefsCallbackResult::from(vec![EntryDef::from(
        ThisWasmEntry::ENTRY_DEFS[0].clone(),
    )]))
}

#[no_mangle]
pub fn __num_entry_types() -> u8 {
    1
}

#[no_mangle]
pub fn __num_link_types() -> u8 {
    0
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreEntry {
            action:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: action, ..
                        },
                    ..
                },
            entry,
        } => match action.app_entry_type() {
            Some(AppEntryType { id, .. }) => {
                if zome_info()?
                    .zome_types
                    .entries
                    .to_local_scope(*id)
                    .filter(|l| l.0 == 0)
                    .is_some()
                {
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
                        action.entry_type()
                    )))
                }
            }
            None => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
