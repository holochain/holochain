/// Error type for holochain_crypto.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// call set_global_crypto_plugin
    PluginNotInitialized,

    /// you already called set_global_crypto_plugin
    PluginAlreadyInitialized,

    /// the output hash size for this call didn't fall within constraints
    BadHashSize,

    /// the key size for this call didn't fall within constraints
    BadKeySize,

    /// the public key size for this call didn't fall within constraints
    BadPublicKeySize,

    /// the secret key size for this call didn't fall within constraints
    BadSecretKeySize,

    /// improper size for seed
    BadSeedSize,

    /// bad bounds for write operation
    WriteOverflow,

    /// Internal libsodium error
    InternalSodium,

    /// error in tokio task
    JoinError(#[from] tokio::task::JoinError),

    /// generic internal error
    Other(String),
}

impl From<&str> for CryptoError {
    fn from(s: &str) -> Self {
        CryptoError::Other(s.into())
    }
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Result type for holochain_crypto.
pub type CryptoResult<T> = Result<T, CryptoError>;
