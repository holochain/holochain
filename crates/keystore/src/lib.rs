#![deny(missing_docs)]
//! holochain_keystore

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

    /// Holochain Crypto Erro.
    #[error("CryptoError: {0}")]
    CryptoError(#[from] holochain_crypto::CryptoError),

    /// Unexpected Internal Error.
    #[error("Other: {0}")]
    Other(String),
}

impl From<String> for KeystoreError {
    fn from(e: String) -> Self {
        KeystoreError::Other(e)
    }
}

impl From<&String> for KeystoreError {
    fn from(e: &String) -> Self {
        e.to_string().into()
    }
}

impl From<&str> for KeystoreError {
    fn from(e: &str) -> Self {
        e.to_string().into()
    }
}

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
            holo_hash::AgentHash
        ),
        ListSignKeys::list_sign_keys (
            "list all the signature public keys this keystore is tracking",
            (),
            Vec<holo_hash::AgentHash>
        ),
        Sign::sign (
            "generate a signature for a given blob of binary data",
            SignInput,
            Signature
        ),
    }
}

/// add signature verification functionality to AgentHash's
/// (because they are actually public keys)
pub trait AgentHashExt {
    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>;
}

impl AgentHashExt for holo_hash::AgentHash {
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;
        use holo_hash::HoloHashCoreHash;

        let result: KeystoreResult<(
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
        )> = (|| {
            let pub_key = holochain_crypto::crypto_insecure_buffer_from_bytes(self.get_bytes())?;
            let signature = holochain_crypto::crypto_insecure_buffer_from_bytes(&signature.0)?;
            let data: SerializedBytes = data.try_into()?;
            let data = holochain_crypto::crypto_insecure_buffer_from_bytes(data.bytes())?;
            Ok((signature, data, pub_key))
        })();

        async move {
            let (mut signature, mut data, mut pub_key) = result?;
            Ok(
                holochain_crypto::crypto_sign_verify(&mut signature, &mut data, &mut pub_key)
                    .await?,
            )
        }
        .boxed()
        .into()
    }
}

pub mod mock_keystore;
