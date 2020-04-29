use crate::*;

/// Input structure for creating a signature.
#[derive(Debug)]
pub struct SignInput {
    /// The public key associated with the private key that should be used to
    /// generate the signature.
    pub key: holo_hash::AgentPubKey,

    /// The data that should be signed.
    pub data: SerializedBytes,
}

impl SignInput {
    /// construct a new SignInput struct.
    pub fn new<D>(key: holo_hash::AgentPubKey, data: D) -> Result<Self, KeystoreError>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        let data: SerializedBytes = data.try_into()?;
        Ok(Self { key, data })
    }

    /// construct a new SignInput struct from raw bytes.
    pub fn new_raw(key: holo_hash::AgentPubKey, data: Vec<u8>) -> Self {
        Self {
            key,
            data: holochain_serialized_bytes::UnsafeBytes::from(data).into(),
        }
    }
}

/// The raw bytes of a signature.
#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct Signature(#[serde(with = "serde_bytes")] pub Vec<u8>);
