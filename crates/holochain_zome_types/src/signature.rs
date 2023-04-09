//! Signature for authenticity of data
use crate::Bytes;
use holo_hash::AgentPubKey;
pub use holochain_integrity_types::signature::*;
use holochain_serialized_bytes::prelude::*;

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

/// Ephemerally sign a vector of bytes (i.e. a `Vec<Vec<u8>>`)
/// Each of the items of the outer vector represents something to sign
/// and will have a corresponding Signature in the output.
/// The public key for the ephemeral operation will be returned in the output.
/// Structurally mirrors/complements the `Signature` struct as a new type.
/// There we know the key on the input side, here we receive the key on the output.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(transparent)]
pub struct SignEphemeral(pub Vec<Bytes>);

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
        Self(datas.into_iter().map(Bytes::from).collect())
    }

    /// Consumes self.
    pub fn into_inner(self) -> Vec<Bytes> {
        self.0
    }
}
