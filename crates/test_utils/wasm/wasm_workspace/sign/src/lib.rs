use hdk3::prelude::*;

#[hdk_extern]
fn sign(sign_input: SignInput) -> ExternResult<Signature> {
    Ok(hdk3::prelude::sign(sign_input.key, sign_input.data)?)
}

#[hdk_extern]
fn verify_signature(
    verify_signature_input: VerifySignatureInput,
) -> ExternResult<VerifySignatureOutput> {
    Ok(VerifySignatureOutput::new(verify_signature!(
        verify_signature_input
    )?))
}
