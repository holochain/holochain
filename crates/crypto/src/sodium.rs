use crate::*;

struct SodiumCryptoPlugin;

impl plugin::CryptoPlugin for SodiumCryptoPlugin {
    fn secure_buffer(&self, size: usize) -> CryptoResult<DynCryptoBytes> {
        // TODO - change this to secure bytes
        Ok(InsecureBytes::new(size))
    }

    fn randombytes_buf<'a, 'b>(
        &'a self,
        buf: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<()>> {
        async move {
            tokio::task::block_in_place(move || unsafe {
                let len = buf.len();
                let mut write_lock = buf.write();
                rust_sodium_sys::randombytes_buf(raw_ptr_void!(write_lock), len);
            });
            Ok(())
        }
        .boxed()
    }

    fn generic_hash_min_bytes(&self) -> usize {
        rust_sodium_sys::crypto_generichash_BYTES_MIN as usize
    }

    fn generic_hash_max_bytes(&self) -> usize {
        rust_sodium_sys::crypto_generichash_BYTES_MAX as usize
    }

    fn generic_hash_key_min_bytes(&self) -> usize {
        rust_sodium_sys::crypto_generichash_KEYBYTES_MIN as usize
    }

    fn generic_hash_key_max_bytes(&self) -> usize {
        rust_sodium_sys::crypto_generichash_KEYBYTES_MAX as usize
    }

    fn generic_hash<'a, 'b>(
        &'a self,
        size: usize,
        data: &'b mut DynCryptoBytes,
        key: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<DynCryptoBytes>> {
        let key_min_bytes = self.generic_hash_key_min_bytes();
        let key_max_bytes = self.generic_hash_key_max_bytes();
        async move {
            tokio::task::block_in_place(move || {
                let mut hash = crypto_insecure_buffer(size)?;
                {
                    let key_lock;
                    let mut key_len = 0_usize;
                    let mut raw_key = std::ptr::null();
                    if let Some(key) = key {
                        key_len = key.len();
                        if key_len < key_min_bytes || key_len > key_max_bytes {
                            return Err(CryptoError::BadKeySize);
                        }
                        key_lock = key.read();
                        raw_key = raw_ptr_char_immut!(key_lock);
                    }

                    let len = data.len();
                    let read_lock = data.read();
                    let mut write_lock = hash.write();

                    unsafe {
                        rust_sodium_sys::crypto_generichash(
                            raw_ptr_char!(write_lock),
                            size,
                            raw_ptr_char_immut!(read_lock),
                            len as libc::c_ulonglong,
                            raw_key,
                            key_len,
                        );
                    }
                }
                Ok(hash)
            })
        }
        .boxed()
    }
}

/// initialize the crypto system plugin with our internal libsodium implementation
pub fn crypto_init_sodium() -> CryptoResult<()> {
    match plugin::set_global_crypto_plugin(Arc::new(SodiumCryptoPlugin)) {
        Ok(_) => {
            unsafe {
                rust_sodium_sys::sodium_init();
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
