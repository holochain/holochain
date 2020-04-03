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
    fn generic_hash<'a, 'b>(
        &'a self,
        size: usize,
        data: &'b mut DynCryptoBytes,
        key: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<DynCryptoBytes>>;
}

/// dyn reference to a crypto plugin
pub type DynCryptoPlugin = Arc<dyn CryptoPlugin + 'static>;

// -- stolen from tracing-core -- //
const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;
static CRYPTO_PLUGIN_INIT: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
static mut CRYPTO_PLUGIN: Option<DynCryptoPlugin> = None;

/// internal get the crypto plugin reference
pub(crate) fn get_global_crypto_plugin() -> CryptoResult<DynCryptoPlugin> {
    if CRYPTO_PLUGIN_INIT.load(Ordering::SeqCst) != INITIALIZED {
        return Err(CryptoError::PluginNotInitialized);
    }
    unsafe {
        // This is safe given the invariant that setting the global dispatcher
        // also sets `GLOBAL_INIT` to `INITIALIZED`.
        Ok(CRYPTO_PLUGIN.as_ref().expect(
            "invariant violated: CRYPTO_PLUGIN must be initialized before CRYPTO_PLUGIN_INIT is set",
        ).clone())
    }
}

/// set the global system crypto plugin
pub fn set_global_crypto_plugin(crypto_plugin: DynCryptoPlugin) -> CryptoResult<()> {
    if CRYPTO_PLUGIN_INIT.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::SeqCst)
        == UNINITIALIZED
    {
        unsafe {
            CRYPTO_PLUGIN = Some(crypto_plugin);
        }
        CRYPTO_PLUGIN_INIT.store(INITIALIZED, Ordering::SeqCst);
        Ok(())
    } else {
        Err(CryptoError::PluginAlreadyInitialized)
    }
}
// -- end stolen from tracing-core -- //
