use crate::prelude::*;

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

/// Verify the passed signature and public key against the passed serializable input.
///
/// The data is not used literally, it is serialized.
/// This is important to use if you have data structures rather than bytes, as the serialization will
/// be passed through the canonical serialization process, guaranteeing consistent behaviour.
/// If you pass in a Vec<u8> expecting it to be verified literally the signature won't verify correctly.
///
/// See [ `verify_signature_raw` ]
pub fn verify_signature<K, S, D>(key: K, signature: S, data: D) -> ExternResult<bool>
where
    K: Into<AgentPubKey>,
    S: Into<Signature>,
    D: serde::Serialize + std::fmt::Debug,
{
    HDK.with(|h| {
        h.borrow()
            .verify_signature(VerifySignature::new(key.into(), signature.into(), data)?)
    })
}

/// Verify the passed signature and public key against the literal bytes input.
///
/// The data is used as-is, there is no serialization or additional processing.
/// This is best to use if you have literal bytes from somewhere.
/// If you pass in a Vec<u8> expecting it to be serialized here, the signature won't verify correctly.
///
/// See [ `verify_signature` ]
pub fn verify_signature_raw<K, S>(key: K, signature: S, data: Vec<u8>) -> ExternResult<bool>
where
    K: Into<AgentPubKey>,
    S: Into<Signature>,
{
    HDK.with(|h| {
        h.borrow()
            .verify_signature(VerifySignature::new_raw(key.into(), signature.into(), data))
    })
}
