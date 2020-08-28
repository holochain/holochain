use hdk3::prelude::*;

#[hdk_extern]
pub fn cap_secret(_: ()) -> ExternResult<CapSecret> {
    Ok(generate_cap_secret!()?)
}

#[hdk_extern]
pub fn transferable_cap_grant(_: ()) -> ExternResult<HeaderHash> {
    Ok(commit_cap_grant!(
        CapGrantEntry {
            access: generate_cap_secret!()?.into(),
            ..Default::default()
        }
    )?)
}
