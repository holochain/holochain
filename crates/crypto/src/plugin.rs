//! traits and types for implementing crypto system plugins

use crate::*;

/// type to implement if you want a new crypto system
pub trait CryptoPlugin: 'static + Send + Sync {
    /// create a new memory-secure buffer
    fn secure_buffer(&self, size: usize) -> CryptoResult<DynCryptoBytes>;

    /// randomize the passed in buffer
    #[must_use]
    fn randombytes_buf<'a, 'b>(
        &'a self,
        buf: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<()>>;

    /// minimum size of output generic (blake2b) hash
    fn generic_hash_min_bytes(&self) -> usize;

    /// maximum size of output generic (blake2b) hash
    fn generic_hash_max_bytes(&self) -> usize;

    /// minimum size of generic hash key
    fn generic_hash_key_min_bytes(&self) -> usize;

    /// maximum size of generic hash key
    fn generic_hash_key_max_bytes(&self) -> usize;

    /// calculate the generic (blake2b) hash of the input data
    #[must_use]
    fn generic_hash_into<'a, 'b>(
        &'a self,
        into_hash: &'b mut DynCryptoBytes,
        data: &'b mut DynCryptoBytes,
        key: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<()>>;

    /// size of seed needed for signature keys
    fn sign_seed_bytes(&self) -> usize;

    /// size of signature public key
    fn sign_public_key_bytes(&self) -> usize;

    /// size of signature secret key
    fn sign_secret_key_bytes(&self) -> usize;

    /// size of an actual signature
    fn sign_bytes(&self) -> usize;

    /// generate a signature keypair optionally based off a seed
    #[must_use]
    fn sign_keypair<'a, 'b>(
        &'a self,
        seed: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<(DynCryptoBytes, DynCryptoBytes)>>;

    /// sign some data
    #[must_use]
    fn sign<'a, 'b>(
        &'a self,
        message: &'b mut DynCryptoBytes,
        secret_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<DynCryptoBytes>>;

    /// verify some signature data
    #[must_use]
    fn sign_verify<'a, 'b>(
        &'a self,
        signature: &'b mut DynCryptoBytes,
        message: &'b mut DynCryptoBytes,
        public_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<bool>>;
}

/// dyn reference to a crypto plugin
pub type DynCryptoPlugin = Arc<dyn CryptoPlugin + 'static>;

static CRYPTO_PLUGIN: OnceCell<DynCryptoPlugin> = OnceCell::new();

/// internal get the crypto plugin reference
pub(crate) fn get_global_crypto_plugin() -> CryptoResult<DynCryptoPlugin> {
    let plugin = CRYPTO_PLUGIN
        .get()
        .ok_or(CryptoError::PluginNotInitialized)?;

    Ok(plugin.clone())
}

/// set the global system crypto plugin
pub fn set_global_crypto_plugin(crypto_plugin: DynCryptoPlugin) -> CryptoResult<()> {
    CRYPTO_PLUGIN
        .set(crypto_plugin)
        .map_err(|_| CryptoError::PluginAlreadyInitialized)
}
