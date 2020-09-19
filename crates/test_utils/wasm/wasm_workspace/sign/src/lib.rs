use hdk3::prelude::*;

#[hdk_extern]
fn sign(sign_input: SignInput) -> ExternResult<Signature> {
    debug!("{:?}", &sign_input.data().bytes())?;
    Ok(sign!(sign_input.key().clone(), sign_input.data().clone())?)
}
