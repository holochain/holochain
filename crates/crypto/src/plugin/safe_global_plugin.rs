use crate::*;
use plugin::DynCryptoPlugin;

static CRYPTO_PLUGIN: OnceCell<DynCryptoPlugin> = OnceCell::new();

/// internal get the crypto plugin reference
pub(crate) fn get() -> CryptoResult<DynCryptoPlugin> {
    let plugin = CRYPTO_PLUGIN
        .get()
        .ok_or(CryptoError::PluginNotInitialized)?;

    Ok(plugin.clone())
}

/// set the global system crypto plugin
pub fn set(crypto_plugin: DynCryptoPlugin) -> CryptoResult<()> {
    CRYPTO_PLUGIN
        .set(crypto_plugin)
        .map_err(|_| CryptoError::PluginAlreadyInitialized)
}
