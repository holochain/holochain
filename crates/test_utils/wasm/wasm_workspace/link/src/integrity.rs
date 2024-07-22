use hdi::prelude::*;

#[hdi_entry_helper]
pub struct Test;

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Test(Test),
}

#[hdi_link_types]
pub enum LinkTypes {
    SomeLinks,
    SomeOtherLinks,
    LinkValidationCallsMustGetValidRecord,
    LinkValidationCallsMustGetActionThenEntry,
    LinkValidationCallsMustGetAgentActivity,
}

pub fn validate_create_link_by_must_get_valid_record(
    base_address: AnyLinkableHash,
) -> ExternResult<ValidateCallbackResult> {
    // Check the entry type for the given action hash
    let action_hash = base_address
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(
            "No action hash associated with link".to_string()
        )))?;
    let _ = must_get_valid_record(action_hash)?;
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_create_link_by_must_get_action_then_entry(
    base_address: AnyLinkableHash,
) -> ExternResult<ValidateCallbackResult> {
    // Check the entry type for the given action hash
    let action_hash = base_address
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(
            "No action hash associated with link".to_string()
        )))?;
    let action = must_get_action(action_hash)?;
    let entry_hash = match action.hashed.into_content() {
        Action::Create(Create { entry_hash, .. }) => entry_hash,
        _ => return Err(wasm_error!(WasmErrorInner::Guest(
            format!("invalid action type")
        ))),
    };
    let _ = must_get_entry(entry_hash)?;
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_create_link_by_must_get_agent_activity(
    base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
) -> ExternResult<ValidateCallbackResult> {
    // Check the entry type for the given action hash
    let agent_pk = target_address
        .into_agent_pub_key()
        .ok_or(wasm_error!(WasmErrorInner::Guest(
            "Invalid target address".to_string()
        )))?;
    let action_hash = base_address
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(
            "No action hash associated with link".to_string()
        )))?;
    let _ = must_get_agent_activity(agent_pk, ChainFilter::new(action_hash))?;
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::RegisterCreateLink {
            base_address,
            target_address,
            link_type,
            ..
        } => match link_type {
            LinkTypes::LinkValidationCallsMustGetValidRecord => {
                validate_create_link_by_must_get_valid_record(base_address)
            }
            LinkTypes::LinkValidationCallsMustGetActionThenEntry => {
                validate_create_link_by_must_get_action_then_entry(base_address)
            }
            LinkTypes::LinkValidationCallsMustGetAgentActivity => {
                validate_create_link_by_must_get_agent_activity(base_address, target_address)
            }
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::StoreRecord(store_record) => match store_record {
            OpRecord::CreateLink {
                base_address,
                target_address,
                link_type,
                ..
            } => match link_type {
                LinkTypes::LinkValidationCallsMustGetValidRecord => {
                    validate_create_link_by_must_get_valid_record(base_address)
                }
                LinkTypes::LinkValidationCallsMustGetActionThenEntry => {
                    validate_create_link_by_must_get_action_then_entry(base_address)
                }
                LinkTypes::LinkValidationCallsMustGetAgentActivity => {
                    validate_create_link_by_must_get_agent_activity(base_address, target_address)
                }
                _ => Ok(ValidateCallbackResult::Valid),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
