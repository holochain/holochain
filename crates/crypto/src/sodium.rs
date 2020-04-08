use crate::*;

mod safe_sodium_buf;
use safe_sodium_buf::S3Buf;

mod safe_sodium;

struct SecureBufferRead<'a>(&'a S3Buf);

impl<'a> Drop for SecureBufferRead<'a> {
    fn drop(&mut self) {
        self.0.set_no_access();
    }
}

impl<'a> std::ops::Deref for SecureBufferRead<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> CryptoBytesRead<'a> for SecureBufferRead<'a> {}

struct SecureBufferWrite<'a>(&'a mut S3Buf);

impl<'a> Drop for SecureBufferWrite<'a> {
    fn drop(&mut self) {
        self.0.set_no_access();
    }
}

impl<'a> std::ops::Deref for SecureBufferWrite<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> std::ops::DerefMut for SecureBufferWrite<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a> CryptoBytesRead<'a> for SecureBufferWrite<'a> {}
impl<'a> CryptoBytesWrite<'a> for SecureBufferWrite<'a> {}

impl CryptoBytes for S3Buf {
    fn clone(&self) -> DynCryptoBytes {
        let mut out = match S3Buf::new(self.s) {
            Err(e) => panic!("{:?}", e),
            Ok(out) => out,
        };
        out.copy_from(0, &self.read()).expect("could not write new");
        Box::new(out)
    }

    fn len(&self) -> usize {
        self.s
    }

    fn is_empty(&self) -> bool {
        self.s == 0
    }

    fn read(&self) -> DynCryptoBytesRead {
        self.set_readable();
        Box::new(SecureBufferRead(self))
    }

    fn write(&mut self) -> DynCryptoBytesWrite {
        self.set_writable();
        Box::new(SecureBufferWrite(self))
    }
}

struct SodiumCryptoPlugin;

impl plugin::CryptoPlugin for SodiumCryptoPlugin {
    fn secure_buffer(&self, size: usize) -> CryptoResult<DynCryptoBytes> {
        Ok(Box::new(S3Buf::new(size)?))
    }

    fn randombytes_buf<'a, 'b>(
        &'a self,
        buf: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<()>> {
        async move {
            tokio::task::block_in_place(move || {
                safe_sodium::randombytes_buf(&mut buf.write());
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

    fn generic_hash_into<'a, 'b>(
        &'a self,
        into_hash: &'b mut DynCryptoBytes,
        data: &'b mut DynCryptoBytes,
        key: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<()>> {
        async move {
            tokio::task::block_in_place(move || {
                {
                    let _tmp;
                    let key: Option<&[u8]> = if let Some(key) = key {
                        _tmp = key.read();
                        Some(&_tmp)
                    } else {
                        None
                    };

                    safe_sodium::crypto_generichash(&mut into_hash.write(), &data.read(), key)?;
                }
                Ok(())
            })
        }
        .boxed()
    }

    fn sign_seed_bytes(&self) -> usize {
        rust_sodium_sys::crypto_sign_SEEDBYTES as usize
    }

    fn sign_public_key_bytes(&self) -> usize {
        rust_sodium_sys::crypto_sign_PUBLICKEYBYTES as usize
    }

    fn sign_secret_key_bytes(&self) -> usize {
        rust_sodium_sys::crypto_sign_SECRETKEYBYTES as usize
    }

    fn sign_bytes(&self) -> usize {
        rust_sodium_sys::crypto_sign_BYTES as usize
    }

    fn sign_keypair<'a, 'b>(
        &'a self,
        seed: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<(DynCryptoBytes, DynCryptoBytes)>> {
        let sec_key = self.secure_buffer(self.sign_secret_key_bytes());
        let pub_key_bytes = self.sign_public_key_bytes();
        async move {
            tokio::task::block_in_place(move || {
                let mut sec_key = sec_key?;
                let mut pub_key = crypto_insecure_buffer(pub_key_bytes)?;

                match seed {
                    Some(seed) => {
                        safe_sodium::crypto_sign_seed_keypair(
                            &mut pub_key.write(),
                            &mut sec_key.write(),
                            &seed.read(),
                        )?;
                    }
                    None => {
                        safe_sodium::crypto_sign_keypair(
                            &mut pub_key.write(),
                            &mut sec_key.write(),
                        )?;
                    }
                }

                Ok((pub_key, sec_key))
            })
        }
        .boxed()
    }

    fn sign<'a, 'b>(
        &'a self,
        message: &'b mut DynCryptoBytes,
        sec_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<DynCryptoBytes>> {
        let sign_bytes = self.sign_bytes();
        async move {
            tokio::task::block_in_place(move || {
                let mut signature = crypto_insecure_buffer(sign_bytes)?;

                safe_sodium::crypto_sign_detached(
                    &mut signature.write(),
                    &message.read(),
                    &sec_key.read(),
                )?;

                Ok(signature)
            })
        }
        .boxed()
    }

    fn sign_verify<'a, 'b>(
        &'a self,
        signature: &'b mut DynCryptoBytes,
        message: &'b mut DynCryptoBytes,
        pub_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<bool>> {
        async move {
            tokio::task::block_in_place(move || {
                safe_sodium::crypto_sign_verify_detached(
                    &signature.read(),
                    &message.read(),
                    &pub_key.read(),
                )
            })
        }
        .boxed()
    }
}

/// initialize the crypto system plugin with our internal libsodium implementation
pub fn crypto_init_sodium() -> CryptoResult<()> {
    match plugin::set_global_crypto_plugin(Arc::new(SodiumCryptoPlugin)) {
        Ok(_) => safe_sodium::sodium_init(),
        Err(e) => Err(e),
    }
}
