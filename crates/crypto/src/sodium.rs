use crate::*;

use libc::c_void;

#[derive(PartialEq)]
enum ProtectState {
    NoAccess,
    ReadOnly,
    ReadWrite,
}

struct SecureBufferRead<'a>(&'a SecureBuffer);

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

struct SecureBufferWrite<'a>(&'a mut SecureBuffer);

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

struct SecureBuffer {
    z: *mut c_void,
    s: usize,
    p: std::cell::RefCell<ProtectState>,
}

// the sodium_malloc c_void is safe to Send
unsafe impl Send for SecureBuffer {}

impl Drop for SecureBuffer {
    fn drop(&mut self) {
        unsafe {
            rust_sodium_sys::sodium_free(self.z);
        }
    }
}

impl std::fmt::Debug for SecureBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self.p.borrow() {
            ProtectState::NoAccess => write!(f, "SecureBuffer( {:?} )", "<NO_ACCESS>"),
            _ => write!(f, "SecureBuffer( {:?} )", *self),
        }
    }
}

impl std::ops::Deref for SecureBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        if *self.p.borrow() == ProtectState::NoAccess {
            panic!("Deref, but state is NoAccess");
        }
        unsafe { &std::slice::from_raw_parts(self.z as *const u8, self.s)[..self.s] }
    }
}

impl std::ops::DerefMut for SecureBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if *self.p.borrow() != ProtectState::ReadWrite {
            panic!("DerefMut, but state is not ReadWrite");
        }
        unsafe { &mut std::slice::from_raw_parts_mut(self.z as *mut u8, self.s)[..self.s] }
    }
}

impl SecureBuffer {
    pub fn new(size: usize) -> Self {
        let z = unsafe {
            // sodium_malloc requires memory-aligned sizes,
            // round up to the nearest 8 bytes.
            let align_size = (size + 7) & !7;
            let z = rust_sodium_sys::sodium_malloc(align_size);
            if z.is_null() {
                panic!("sodium_malloc could not allocate");
            }
            rust_sodium_sys::sodium_memzero(z, align_size);
            rust_sodium_sys::sodium_mprotect_noaccess(z);
            z
        };

        SecureBuffer {
            z,
            s: size,
            p: std::cell::RefCell::new(ProtectState::NoAccess),
        }
    }

    fn set_no_access(&self) {
        if *self.p.borrow() == ProtectState::NoAccess {
            panic!("already no access... bad logic");
        }
        unsafe {
            rust_sodium_sys::sodium_mprotect_noaccess(self.z);
        }
        *self.p.borrow_mut() = ProtectState::NoAccess;
    }

    fn set_readable(&self) {
        if *self.p.borrow() != ProtectState::NoAccess {
            panic!("not no access... bad logic");
        }
        unsafe {
            rust_sodium_sys::sodium_mprotect_readonly(self.z);
        }
        *self.p.borrow_mut() = ProtectState::ReadOnly;
    }

    fn set_writable(&self) {
        if *self.p.borrow() != ProtectState::NoAccess {
            panic!("not no access... bad logic");
        }
        unsafe {
            rust_sodium_sys::sodium_mprotect_readwrite(self.z);
        }
        *self.p.borrow_mut() = ProtectState::ReadWrite;
    }
}

impl CryptoBytes for SecureBuffer {
    fn clone(&self) -> DynCryptoBytes {
        let mut out = SecureBuffer::new(self.s);
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
        Ok(Box::new(SecureBuffer::new(size)))
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

    fn generic_hash_into<'a, 'b>(
        &'a self,
        into_hash: &'b mut DynCryptoBytes,
        data: &'b mut DynCryptoBytes,
        key: Option<&'b mut DynCryptoBytes>,
    ) -> BoxFuture<'b, CryptoResult<()>> {
        let hash_min_bytes = self.generic_hash_min_bytes();
        let hash_max_bytes = self.generic_hash_max_bytes();
        let key_min_bytes = self.generic_hash_key_min_bytes();
        let key_max_bytes = self.generic_hash_key_max_bytes();
        async move {
            tokio::task::block_in_place(move || {
                let hash_len = into_hash.len();
                if hash_len < hash_min_bytes || hash_len > hash_max_bytes {
                    return Err(CryptoError::BadHashSize);
                }

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
                    let mut write_lock = into_hash.write();

                    unsafe {
                        rust_sodium_sys::crypto_generichash(
                            raw_ptr_char!(write_lock),
                            hash_len,
                            raw_ptr_char_immut!(read_lock),
                            len as libc::c_ulonglong,
                            raw_key,
                            key_len,
                        );
                    }
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
        let seed_bytes = self.sign_seed_bytes();
        async move {
            tokio::task::block_in_place(move || {
                let mut sec_key = sec_key?;
                let mut pub_key = crypto_insecure_buffer(pub_key_bytes)?;

                match seed {
                    Some(seed) => {
                        if seed.len() != seed_bytes {
                            return Err(CryptoError::BadSeedSize);
                        }
                        let mut pub_key = pub_key.write();
                        let mut sec_key = sec_key.write();
                        let seed = seed.read();
                        unsafe {
                            if rust_sodium_sys::crypto_sign_seed_keypair(
                                raw_ptr_char!(pub_key),
                                raw_ptr_char!(sec_key),
                                raw_ptr_char_immut!(seed),
                            ) != 0 as libc::c_int
                            {
                                return Err("keypair failed".into());
                            }
                        }
                    }
                    None => {
                        let mut pub_key = pub_key.write();
                        let mut sec_key = sec_key.write();
                        unsafe {
                            if rust_sodium_sys::crypto_sign_keypair(
                                raw_ptr_char!(pub_key),
                                raw_ptr_char!(sec_key),
                            ) != 0 as libc::c_int
                            {
                                return Err("keypair failed".into());
                            }
                        }
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
        secret_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<DynCryptoBytes>> {
        let sign_bytes = self.sign_bytes();
        let sec_key_bytes = self.sign_secret_key_bytes();
        async move {
            tokio::task::block_in_place(move || {
                if secret_key.len() != sec_key_bytes {
                    return Err(CryptoError::BadSecretKeySize);
                }
                let mut signature = crypto_insecure_buffer(sign_bytes)?;

                {
                    let message_len = message.len();
                    let message = message.read();
                    let secret_key = secret_key.read();
                    let mut signature = signature.write();

                    unsafe {
                        if rust_sodium_sys::crypto_sign_detached(
                            raw_ptr_char!(signature),
                            std::ptr::null_mut(),
                            raw_ptr_char_immut!(message),
                            message_len as libc::c_ulonglong,
                            raw_ptr_char_immut!(secret_key),
                        ) != 0 as libc::c_int
                        {
                            return Err("signature failed".into());
                        }
                    }
                }

                Ok(signature)
            })
        }
        .boxed()
    }

    fn sign_verify<'a, 'b>(
        &'a self,
        signature: &'b mut DynCryptoBytes,
        message: &'b mut DynCryptoBytes,
        public_key: &'b mut DynCryptoBytes,
    ) -> BoxFuture<'b, CryptoResult<bool>> {
        let pub_key_bytes = self.sign_public_key_bytes();
        async move {
            tokio::task::block_in_place(move || {
                if public_key.len() != pub_key_bytes {
                    return Err(CryptoError::BadPublicKeySize);
                }

                let signature = signature.read();
                let message_len = message.len();
                let message = message.read();
                let public_key = public_key.read();

                Ok(unsafe {
                    rust_sodium_sys::crypto_sign_verify_detached(
                        raw_ptr_char_immut!(signature),
                        raw_ptr_char_immut!(message),
                        message_len as libc::c_ulonglong,
                        raw_ptr_char_immut!(public_key),
                    )
                } == 0 as libc::c_int)
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
