use crate::prelude::*;

/// Verify the passed signature and public key against the passed data
pub fn verify_signature<K: Into<AgentPubKey>, S: Into<Signature>, D: Into<SerializedBytes>>(
    key: K,
    signature: S,
    data: D,
) -> ExternResult<bool> {
    host_call::<VerifySignature, bool>(
        __verify_signature,
        VerifySignature {
            key: key.into(),
            signature: signature.into(),
            data: data.into(),
        },
    )
}
