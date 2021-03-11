//! Signature for authenticity of data
use crate::Bytes;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// Ed25519 signatures are always the same length, 64 bytes.
pub const SIGNATURE_BYTES: usize = 64;

/// Input structure for creating a signature.
#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct Sign {
    /// The public key associated with the private key that should be used to
    /// generate the signature.
    pub key: holo_hash::AgentPubKey,

    /// The data that should be signed.
    pub data: Bytes,
}

impl Sign {
    /// construct a new Sign struct.
    pub fn new<S>(key: holo_hash::AgentPubKey, input: S) -> Result<Self, SerializedBytesError>
    where
        S: Serialize + std::fmt::Debug,
    {
        Ok(Self::new_raw(
            key,
            holochain_serialized_bytes::encode(&input)?,
        ))
    }

    /// construct a new Sign struct from raw bytes.
    pub fn new_raw(key: holo_hash::AgentPubKey, data: Vec<u8>) -> Self {
        Self {
            key,
            data: Bytes::from(data),
        }
    }

    /// key getter
    pub fn key(&self) -> &AgentPubKey {
        &self.key
    }

    /// data getter
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// The raw bytes of a signature.
#[derive(Clone, Hash, PartialOrd, Ord)]
pub struct Signature(pub [u8; SIGNATURE_BYTES]);

// This is more for convenience/convention that being worried
// about things like constant time equality.
// Signature verification should always defer to the host.
// What's nice about this is that we can easily handle fixed size signatures.
crate::secure_primitive!(Signature, SIGNATURE_BYTES);

/// Ephemerally sign a vector of bytes (i.e. a Vec<Vec<u8>>)
/// Each of the items of the outer vector represents something to sign
/// and will have a corresponding Signature in the output.
/// The public key for the ephemeral operation will be returned in the output.
/// Structurally mirrors/complements the `Signature` struct as a new type.
/// There we know the key on the input side, here we receive the key on the output.
#[derive(Serialize, Deserialize, Debug)]
pub struct SignEphemeral(Vec<Bytes>);

impl SignEphemeral {
    /// Construct a new SignEphemeral from a vector of Serialize inputs.
    /// The signing key will be generated and discarded by the host.
    pub fn new<S>(inputs: Vec<S>) -> Result<Self, SerializedBytesError>
    where
        S: Serialize + std::fmt::Debug,
    {
        let datas: Result<Vec<_>, _> = inputs
            .into_iter()
            .map(|s| holochain_serialized_bytes::encode(&s))
            .collect();
        Ok(Self::new_raw(datas?))
    }

    /// Construct a SignEphemeral from a vector of bytes.
    pub fn new_raw(datas: Vec<Vec<u8>>) -> Self {
        Self(datas.into_iter().map(|d| Bytes::from(d)).collect())
    }

    /// Consumes self.
    pub fn into_inner(self) -> Vec<Bytes> {
        self.0
    }
}

/// The output of ephemeral signing.
/// The private key for this public key has been discarded by this point.
/// The signatures match the public key provided but cannot be reproduced
/// or forged because the private key no longer exists.
/// The signatures match the input items positionally in the vector,
/// it is up to the caller to reconstruct/align/zip them back together.
#[derive(Serialize, Deserialize, Debug)]
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
        &self.data.as_ref()
    }

    /// Alias for as_ref for signature.
    pub fn as_signature_ref(&self) -> &Signature {
        &self.as_ref()
    }

    /// Alias for as_ref for agent key.
    pub fn as_key_ref(&self) -> &holo_hash::AgentPubKey {
        &self.as_ref()
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
