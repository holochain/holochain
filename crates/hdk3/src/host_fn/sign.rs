use crate::prelude::*;

/// Sign some data using the private key for the passed public key
///
/// Assuming the private key for the provided
pub fn sign(key: AgentPubKey, data: SerializedBytes) -> HdkResult<Signature> {
    Ok(
        host_call::<SignInput, SignOutput>(__sign, SignInput::new(Sign { key, data }))?
            .into_inner(),
    )
}
