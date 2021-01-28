use crate::prelude::*;

/// Sign some data using the private key for the passed public key
///
/// Assuming the private key for the provided
pub fn sign(key: AgentPubKey, data: Vec<u8>) -> ExternResult<Signature> {
    host_call::<Sign, Signature>(__sign, Sign { key, data })
}
