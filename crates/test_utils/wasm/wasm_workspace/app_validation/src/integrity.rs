// pub mod post;
use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Post(String);
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Post(Post),
}
#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AllPosts,
    PostsByAuthor,
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::StoreRecord(store_record) => {
            match store_record {
                OpRecord::CreateLink { target_address, .. } => {
                    let action_hash = target_address.into_action_hash().ok_or(wasm_error!(
                        WasmErrorInner::Guest("No action hash associated with link".to_string())
                    ))?;
                    let record = must_get_valid_record(action_hash)?;
                    let _post: Post = record
                        .entry()
                        .to_app_option()
                        .map_err(|e| wasm_error!(e))?
                        .ok_or(wasm_error!(WasmErrorInner::Guest(
                            "Linked action must reference an entry".to_string()
                        )))?;
                    Ok(ValidateCallbackResult::Valid)
                }
                _ => Ok(ValidateCallbackResult::Valid),
            }
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
