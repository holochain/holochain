/// Error type for holochain_crypto.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// call set_global_crypto_plugin
    PluginNotInitialized,

    /// you already called set_global_crypto_plugin
    PluginAlreadyInitialized,

    /// the key size for this call didn't fall within constraints
    BadKeySize,

    /// bad bounds for write operation
    WriteOverflow,

    /// error in tokio task
    JoinError(#[from] tokio::task::JoinError),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Result type for holochain_crypto.
pub type CryptoResult<T> = Result<T, CryptoError>;
