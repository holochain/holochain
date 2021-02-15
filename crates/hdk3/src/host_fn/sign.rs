use crate::prelude::*;

/// Sign something that is serializable using the private key for the passed public key.
pub fn sign<K, D>(key: K, data: D) -> ExternResult<Signature>
where
    K: Into<AgentPubKey>,
    D: serde::Serialize + std::fmt::Debug,
{
    host_call::<Sign, Signature>(__sign, Sign::new(key.into(), data)?)
}

/// Sign some data using the private key for the passed public key.
///
/// Assuming the private key for the provided pubkey exists in lair this will work.
/// If we don't have the private key for the public key then we can't sign anything!
///
/// See [`sign`](fn@sign)
pub fn sign_raw<K>(key: K, data: Vec<u8>) -> ExternResult<Signature>
where
    K: Into<AgentPubKey>,
{
    host_call::<Sign, Signature>(__sign, Sign::new_raw(key.into(), data))
}
