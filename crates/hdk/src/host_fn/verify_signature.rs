use crate::prelude::*;

/// Verify the passed signature and public key against the passed data
pub fn verify_signature<'a, K: Into<AgentPubKey>, S: Into<Signature>, D: Into<SerializedBytes>>(
    key: K,
    signature: S,
    data: D,
) -> HdkResult<bool> {
    host_externs!(__verify_signature);
    host_fn!(
        __verify_signature,
        VerifySignatureInput {
            key: key.into(),
            signature: signature.into(),
            data: data.into(),
        },
        VerifySignatureOutput
    )
}
