use hdi::prelude::*;

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<(), ()>()? {
        FlatOp::StoreRecord(store_record) => match store_record {
            OpRecord::DeleteEntry {
                ..
            } => Ok(ValidateCallbackResult::Invalid("This zome does not define any entry types".to_string())),
            OpRecord::DeleteLink {
                ..
            } => Ok(ValidateCallbackResult::Invalid("This zome does not define any link types".to_string())),
            _ => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
