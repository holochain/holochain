use hdk3::prelude::*;

#[hdk_extern]
fn random_bytes(bytes: u32) -> ExternResult<Bytes> {
    debug!(bytes = bytes);
    Ok(hdk3::prelude::random_bytes(bytes)?)
}
