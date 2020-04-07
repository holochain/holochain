//! This code is stolen from tracing-core,
//! the only modifications are naming.

use crate::*;
use plugin::DynCryptoPlugin;

// -- stolen from tracing-core -- //

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;
static CRYPTO_PLUGIN_INIT: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
static mut CRYPTO_PLUGIN: Option<DynCryptoPlugin> = None;

/// internal get the crypto plugin reference
pub(crate) fn get() -> CryptoResult<DynCryptoPlugin> {
    if CRYPTO_PLUGIN_INIT.load(Ordering::SeqCst) != INITIALIZED {
        return Err(CryptoError::PluginNotInitialized);
    }

    // INVARIANTS:
    //   - CRYPTO_PLUGIN_INIT:
    //     - we only get to this point if we are INITIALIZED
    //     - INITIALIZED is only set by `set` after the plugin is set
    unsafe {
        Ok(CRYPTO_PLUGIN.as_ref().expect(
            "invariant violated: CRYPTO_PLUGIN must be initialized before CRYPTO_PLUGIN_INIT is set",
        ).clone())
    }
}

/// set the global system crypto plugin
pub fn set(crypto_plugin: DynCryptoPlugin) -> CryptoResult<()> {
    if CRYPTO_PLUGIN_INIT.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::SeqCst)
        == UNINITIALIZED
    {
        // INVARIANTS:
        //   - this is the only function that sets CRYPTO_PLUGIN_INIT
        //   - CRYPTO_PLUGIN_INIT defaults to UNINITIALIZED
        //   - we only set it if we can swap in INITIALIZING over UNINITIALIZED
        //   - we only set INITIALIZED after we set the plugin
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
