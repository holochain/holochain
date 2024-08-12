use hdi::prelude::*;

#[hdk_entry_helper]
pub struct A;

/// Entry type enum for hc demo-cli.
#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    A(A),
}

/// Link type enum for hc demo-cli.
#[hdk_link_types]
pub enum LinkTypes {
    T,
}

/// Reproduction of:
/// https://github.com/Holo-Host/holofuel/blob/65308fa37ee77407bbc274ffeb11aaef8e844ce0/zomes/transactor_integrity/src/states/mod.rs#L31
#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    // Op::StoreEntry(e) => Ok(validate_create(
    //     h,
    //     e.action.hashed.author().clone(),
    //     e.action.to_hash(),
    // )?),
    // Op::StoreRecord(e) => Ok(validate_create(
    //     h,
    //     e.record.action().author().clone(),
    //     e.record.action().to_hash(),
    // )?),
    // _ => Ok(ValidateCallbackResult::Valid),
    if let Some(hash) = op.prev_action() {
        tracing::info!("op: {:?} {}", op.action_type(), op.action_seq());
        let activity =
            must_get_agent_activity(op.author().clone(), ChainFilter::new(hash.clone()))?;

        let _rs: Vec<_> = activity
            .iter()
            .filter_map(|a| must_get_valid_record(a.action.action_address().clone()).ok())
            .collect();
        tracing::info!("rs: {}", _rs.len());
    }
    Ok(ValidateCallbackResult::Valid)
}
