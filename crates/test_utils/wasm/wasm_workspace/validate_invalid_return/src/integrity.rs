use hdk::prelude::*;

#[hdk_extern]
fn validate(op: Op) -> ExternResult<usize> {
    Ok(42)
}

