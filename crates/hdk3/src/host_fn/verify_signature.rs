use crate::prelude::*;

/// Verify the passed signature and public key against the passed data
pub fn verify_signature<K: Into<AgentPubKey>, S: Into<Signature>, D: Into<SerializedBytes>>(
    key: K,
    signature: S,
    data: D,
) -> HdkResult<bool> {
    Ok(host_call::<VerifySignatureInput, VerifySignatureOutput>(
        __verify_signature,
        &VerifySignatureInput::new(VerifySignature {
            key: key.into(),
            signature: signature.into(),
            data: data.into(),
        }),
    )?
    .into_inner())
}
