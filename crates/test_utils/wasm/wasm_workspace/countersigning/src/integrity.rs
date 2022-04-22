use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub enum Thing {
    Valid,
    Invalid,
}

impl From<Thing> for ValidateCallbackResult {
    fn from(thing: Thing) -> Self {
        match thing {
            Thing::Valid => ValidateCallbackResult::Valid,
            Thing::Invalid => ValidateCallbackResult::Invalid("never valid".to_string()),
        }
    }
}

#[hdk_entry_defs]
pub enum EntryTypes {
    Thing(Thing),
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    let this_zome = zome_info()?;
    match op {
        Op::StoreElement { element }
            if element.header().entry_type().map_or(false, |et| match et {
                EntryType::App(app_entry_type) => this_zome.matches_entry_def_id(
                    app_entry_type,
                    EntryTypes::variant_to_entry_def_id(EntryTypes::Thing),
                ),
                _ => false,
            }) =>
        {
            Ok(Thing::try_from(element)?.into())
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
