use hdk::prelude::*;

#[hdk_extern]
fn random_bytes(bytes: u32) -> ExternResult<Bytes> {
    Ok(hdk::prelude::random_bytes(bytes)?)
}
