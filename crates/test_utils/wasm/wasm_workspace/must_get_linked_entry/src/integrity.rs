use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Thing {
    pub content: u32,
}

#[derive(Serialize, Deserialize)]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    #[entry_type(visibility = "private")]
    Thing(Thing),
}

#[hdk_link_types]
pub enum LinkTypes {
    SomeLink,
}

fn validate_create_thing(action: EntryCreationAction) -> ExternResult<ValidateCallbackResult> {
    let author = action.author().clone();
    if let EntryCreationAction::Create(action) = action {
        let action_hash = hash_action(action.into_action().clone())?;
        let filter = ChainFilter::new(action_hash);
        let result = must_get_agent_activity(author.clone(), filter)?;
        debug!("Agent Activity Count: {}", result.len());
    }
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::StoreEntry(store_entry) => match store_entry {
            OpEntry::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::Thing(_) => validate_create_thing(EntryCreationAction::Create(action)),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::StoreRecord(store_record) => match store_record {
            OpRecord::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::Thing(_) => validate_create_thing(EntryCreationAction::Create(action)),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterCreateLink {
            target_address,
            link_type,
            ..
        } => {
            validate_thing_some_link(
                target_address,
                link_type,
            )
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

fn validate_thing_some_link(
    target_address: AnyLinkableHash,
    link_type: LinkTypes,
) -> ExternResult<ValidateCallbackResult> {
    let entry_hash = match target_address.clone().try_into() {
        Ok(entry_hash) => entry_hash,
        Err(_) => {
            return Ok(ValidateCallbackResult::Invalid(format!(
                "The target address for {:?} must be an entry hash",
                link_type
            )));
        }
    };
    tracing::info!("Searching for target address {:?}", entry_hash);
    let entry = must_get_entry(entry_hash)?;
    tracing::info!("Got entry {:?}", entry);
    match entry.as_app_entry() {
        Some(app_entry) => {
            match <SerializedBytes as TryInto<Thing>>::try_into(
                app_entry.clone().into_sb(),
            ) {
                Ok(_) => Ok(ValidateCallbackResult::Valid),
                Err(_) => Ok(ValidateCallbackResult::Invalid(format!(
                    "The target for {:?} must be a {}",
                    link_type,
                    std::any::type_name::<Thing>()
                ))),
            }
        }
        None => Ok(ValidateCallbackResult::Invalid(format!(
            "The target for {:?} must be an app entry",
            link_type
        ))),
    }
}
