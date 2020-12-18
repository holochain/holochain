use hdk3::prelude::*;

#[hdk_extern]
fn sign(sign_input: Sign) -> ExternResult<Signature> {
    Ok(hdk3::prelude::sign(sign_input.key, sign_input.data)?)
}

#[hdk_extern]
fn verify_signature(
    verify_signature_input: VerifySignature,
) -> ExternResult<VerifySignatureOutput> {
    let VerifySignature {
        key,
        signature,
        data,
    } = verify_signature_input;
    Ok(VerifySignatureOutput::new(hdk3::prelude::verify_signature(
        key, signature, data,
    )?))
}
