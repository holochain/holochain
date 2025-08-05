use hdk::prelude::*;

#[hdk_extern]
fn echo() -> ExternResult<String> {
    Ok("2".to_string())
}
