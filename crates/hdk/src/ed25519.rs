use crate::prelude::*;

pub use holochain_deterministic_integrity::ed25519::*;

/// Sign something that is serializable using the private key for the passed public key.
///
/// Serde convenience for [ `sign_raw `].
pub fn sign<K, D>(key: K, data: D) -> ExternResult<Signature>
where
    K: Into<AgentPubKey>,
    D: serde::Serialize + std::fmt::Debug,
{
    HDK.with(|h| h.borrow().sign(Sign::new(key.into(), data)?))
}

/// Sign some data using the private key for the passed public key.
///
/// Assuming the private key for the provided pubkey exists in lair this will work.
/// If we don't have the private key for the public key then we can't sign anything!
///
/// See [ `sign` ]
pub fn sign_raw<K>(key: K, data: Vec<u8>) -> ExternResult<Signature>
where
    K: Into<AgentPubKey>,
{
    HDK.with(|h| h.borrow().sign(Sign::new_raw(key.into(), data)))
}

/// Sign N serializable things using an ephemeral private key.
///
/// Serde convenience for [ `sign_ephemeral_raw` ].
pub fn sign_ephemeral<D>(datas: Vec<D>) -> ExternResult<EphemeralSignatures>
where
    D: serde::Serialize + std::fmt::Debug,
{
    HDK.with(|h| h.borrow().sign_ephemeral(SignEphemeral::new(datas)?))
}

/// Sign N data using an ephemeral private key.
///
/// This is a complement to [ `sign_raw` ] in case we don't have a meaningful key for the input.
/// __The generated private half of the key is discarded immediately upon signing__.
///
/// The signatures output are pairwise ordered the same as the input data.
/// It is up to the caller to construct meaning for ephemeral signatures in some cryptographic system.
pub fn sign_ephemeral_raw(datas: Vec<Vec<u8>>) -> ExternResult<EphemeralSignatures> {
    HDK.with(|h| h.borrow().sign_ephemeral(SignEphemeral::new_raw(datas)))
}
