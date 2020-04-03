use crate::*;

/// create a new insecure byte buffer
pub fn crypto_insecure_buffer(size: usize) -> CryptoResult<DynCryptoBytes> {
    Ok(InsecureBytes::new(size))
}

/// create a new secure byte buffer (i.e. for use with private keys)
pub fn crypto_secure_buffer(size: usize) -> CryptoResult<DynCryptoBytes> {
    plugin::get_global_crypto_plugin()?.secure_buffer(size)
}

/// randomize a byte buffer
pub async fn crypto_randombytes_buf(buf: &mut DynCryptoBytes) -> CryptoResult<()> {
    plugin::get_global_crypto_plugin()?
        .randombytes_buf(buf)
        .await
}

/// minimum size of output generic (blake2b) hash
pub fn crypto_generic_hash_min_bytes() -> CryptoResult<usize> {
    Ok(plugin::get_global_crypto_plugin()?.generic_hash_min_bytes())
}

/// maximum size of output generic (blake2b) hash
pub fn crypto_generic_hash_max_bytes() -> CryptoResult<usize> {
    Ok(plugin::get_global_crypto_plugin()?.generic_hash_max_bytes())
}

/// minimum size of generic hash key
pub fn crypto_generic_hash_key_min_bytes() -> CryptoResult<usize> {
    Ok(plugin::get_global_crypto_plugin()?.generic_hash_key_min_bytes())
}

/// maximum size of generic hash key
pub fn crypto_generic_hash_key_max_bytes() -> CryptoResult<usize> {
    Ok(plugin::get_global_crypto_plugin()?.generic_hash_key_max_bytes())
}

/// calculate the generic (blake2b) hash for the given data
/// with the optional blake2b key
pub async fn crypto_generic_hash(
    size: usize,
    data: &mut DynCryptoBytes,
    key: Option<&mut DynCryptoBytes>,
) -> CryptoResult<DynCryptoBytes> {
    plugin::get_global_crypto_plugin()?
        .generic_hash(size, data, key)
        .await
}
