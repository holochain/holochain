//! Signature for authenticity of data
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// Ed25519 signatures are always the same length, 64 bytes.
pub const SIGNATURE_BYTES: usize = 64;

/// The raw bytes of a signature.
#[derive(Clone, PartialOrd, Hash, Ord)]
/// The equality is not different, it's just constant time, so we can derive a hash.
/// For an actually secure thing we wouldn't want to just assume a safe default hashing
/// But that is not what clippy is complaining about here.
#[allow(clippy::derive_hash_xor_eq)]
pub struct Signature(pub [u8; SIGNATURE_BYTES]);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Signature {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut buf = [0; SIGNATURE_BYTES];
        u.fill_buffer(&mut buf)?;
        Ok(Signature(buf))
    }
}

// This is more for convenience/convention that being worried
// about things like constant time equality.
// Signature verification should always defer to the host.
// What's nice about this is that we can easily handle fixed size signatures.
crate::secure_primitive!(Signature, SIGNATURE_BYTES);

/// The output of ephemeral signing.
/// The private key for this public key has been discarded by this point.
/// The signatures match the public key provided but cannot be reproduced
/// or forged because the private key no longer exists.
/// The signatures match the input items positionally in the vector,
/// it is up to the caller to reconstruct/align/zip them back together.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EphemeralSignatures {
    /// The public key associated with the now-discarded private key used to sign.
    pub key: holo_hash::AgentPubKey,
    /// The signatures for the input data to be matched in order, pairwise.
    pub signatures: Vec<Signature>,
}

/// Mirror struct for Sign that includes a signature to verify against a key and data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct VerifySignature {
    /// The public key associated with the private key that should be used to
    /// verify the signature.
    pub key: holo_hash::AgentPubKey,

    /// The signature being verified.
    pub signature: Signature,

    /// The signed data
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl AsRef<Signature> for VerifySignature {
    fn as_ref(&self) -> &Signature {
        &self.signature
    }
}

impl AsRef<holo_hash::AgentPubKey> for VerifySignature {
    fn as_ref(&self) -> &AgentPubKey {
        &self.key
    }
}

impl VerifySignature {
    /// Alias for as_ref for data.
    pub fn as_data_ref(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Alias for as_ref for signature.
    pub fn as_signature_ref(&self) -> &Signature {
        self.as_ref()
    }

    /// Alias for as_ref for agent key.
    pub fn as_key_ref(&self) -> &holo_hash::AgentPubKey {
        self.as_ref()
    }

    /// construct a new VerifySignature struct.
    pub fn new<D>(
        key: holo_hash::AgentPubKey,
        signature: Signature,
        data: D,
    ) -> Result<Self, SerializedBytesError>
    where
        D: serde::Serialize + std::fmt::Debug,
    {
        Ok(Self {
            key,
            signature,
            data: holochain_serialized_bytes::encode(&data)?,
        })
    }

    /// construct a new Sign struct from raw bytes.
    pub fn new_raw(key: holo_hash::AgentPubKey, signature: Signature, data: Vec<u8>) -> Self {
        Self {
            key,
            signature,
            data,
        }
    }
}
