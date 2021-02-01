use hdk3::prelude::*;

#[hdk_extern]
fn random_bytes(bytes: RandomBytesInput) -> ExternResult<RandomBytesOutput> {
    Ok(RandomBytesOutput::new(Bytes::from(
        hdk3::prelude::random_bytes(bytes.into_inner())?,
    )))
}
