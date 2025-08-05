use hdk::prelude::*;

#[hdk_extern]
fn echo() -> ExternResult<String> {
    Ok("1".to_string())
}
