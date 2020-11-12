use crate::prelude::*;

/// Sign some data using the private key for the passed public key
///
/// Assuming the private key for the provided
pub fn sign(key: AgentPubKey, data: SerializedBytes) -> HdkResult<Signature> {
    host_externs!(__sign);
    host_fn!(
        __sign,
        SignInput {
            key,
            data,
        },
        SignOutput
    )
}
