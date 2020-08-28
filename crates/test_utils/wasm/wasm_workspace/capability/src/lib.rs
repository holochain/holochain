use hdk3::prelude::*;

#[hdk_extern]
pub fn cap_secret(_: ()) -> ExternResult<CapSecret> {
    Ok(generate_cap_secret!()?)
}

#[hdk_extern]
pub fn transferable_cap_grant(secret: CapSecret) -> ExternResult<HeaderHash> {
    Ok(commit_cap_grant!(
        CapGrantEntry {
            access: secret.into(),
            ..Default::default()
        }
    )?)
}

#[hdk_extern]
fn get_entry(header_hash: HeaderHash) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get!(header_hash)?))
}
