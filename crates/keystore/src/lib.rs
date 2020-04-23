#![deny(missing_docs)]
//! holochain_keystore

/// re-exported dependencies
pub mod dependencies {
    pub use ghost_actor::dependencies::must_future;
}

use dependencies::must_future::MustBoxFuture;

use holochain_serialized_bytes::prelude::*;

/// Keystore Error Type.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// An error generated from the GhostActor system.
    #[error("GhostError: {0}")]
    GhostError(#[from] ghost_actor::GhostError),

    /// Error serializing data.
    #[error("SerializedBytesError: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),
}

/// Keystore Result Type.
pub type KeystoreResult<T> = Result<T, KeystoreError>;

/// Keystore Future Type.
pub type KeystoreFuture<T> = MustBoxFuture<'static, KeystoreResult<T>>;

/// Input structure for creating a signature.
#[derive(Debug)]
pub struct SignInput {
    /// The public key associated with the private key that should be used to
    /// generate the signature.
    pub key: holo_hash::AgentHash,

    /// The data that should be signed.
    pub data: SerializedBytes,
}

impl SignInput {
    /// construct a new SignInput struct.
    pub fn new<D>(key: holo_hash::AgentHash, data: D) -> Result<Self, KeystoreError>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        let data: SerializedBytes = data.try_into()?;
        Ok(Self { key, data })
    }
}

/// The raw bytes of a signature.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Signature(#[serde(with = "serde_bytes")] pub Vec<u8>);

ghost_actor::ghost_actor! {
    name: pub Keystore,
    error: KeystoreError,
    api: {
        GenerateSignKeypairFromPureEntropy::generate_sign_keypair_from_pure_entropy (
            "generates a new pure entropy keypair in the keystore, returning the public key",
            (),
            KeystoreFuture<holo_hash::AgentHash>
        ),
        ListSignKeys::list_sign_keys (
            "list all the signature public keys this keystore is tracking",
            (),
            KeystoreFuture<Vec<holo_hash::AgentHash>>
        ),
        Sign::sign (
            "generate a signature for a given blob of binary data",
            SignInput,
            KeystoreFuture<Signature>
        ),
    }
}
