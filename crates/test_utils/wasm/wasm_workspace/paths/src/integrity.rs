use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(PartialEq, Eq)]
pub struct BookEntry {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    BookEntry(BookEntry),
}

#[hdk_link_types]
pub enum LinkTypes {
    AuthorPath,
    AuthorBook,
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::StoreRecord(OpRecord::CreateLink {
            link_type: LinkTypes::AuthorPath,
            base_address,
            target_address,
            tag,
            ..
        }) => {
            let tag_bytes = SerializedBytes::from(UnsafeBytes::from(tag.clone().into_inner()));

            if base_address.clone().into_entry_hash().is_none() {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's base_address '{base_address}' was not a valid entry hash",
                )))
            } else if target_address.clone().into_entry_hash().is_none() {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's target_address '{target_address}' was not a valid entry hash",
                )))
            } else if let Ok(tag_components) = Component::try_from(tag_bytes) {
                if let Ok(tag_string) = String::try_from(&tag_components) {
                    if tag_string
                        .chars()
                        .all(|c| c == '-' || c.is_ascii_lowercase())
                    {
                        Ok(ValidateCallbackResult::Valid)
                    } else {
                        Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's tag of '{tag_string:?}' contained more than lower-case ASCII letters and dashes",
                )))
                    }
                } else {
                    Ok(ValidateCallbackResult::Invalid(format!(
                        "The components of the link's tag '{tag_components:?}' were not valid strings",
                    )))
                }
            } else {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's tag '{tag:?}' was not a path component",
                )))
            }
        }
        FlatOp::StoreRecord(OpRecord::CreateLink {
            link_type: LinkTypes::AuthorBook,
            base_address,
            target_address,
            tag,
            ..
        }) => {
            if TryInto::<String>::try_into(tag.clone()).is_err() {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's tag of '{tag:?}' was not a valid string",
                )))
            } else if base_address.clone().into_entry_hash().is_none() {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's base_address '{base_address}' was not a valid entry hash",
                )))
            } else if let Some(book_entry_hash) = target_address.clone().into_entry_hash() {
                if must_get_entry(book_entry_hash).is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::Invalid(format!(
                        "Link's target_address '{target_address}' does not point to an entry",
                    )))
                }
            } else {
                Ok(ValidateCallbackResult::Invalid(format!(
                    "Link's target_address '{target_address}' was not a valid entry hash",
                )))
            }
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
