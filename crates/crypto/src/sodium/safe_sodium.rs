//! None of the unsafe blocks in here interdepend.
//! The invariant lists for each unsafe block can be evaluated separately.

use crate::*;

pub(crate) fn sodium_init() -> CryptoResult<()> {
    // sodium_init will return < 0 if an allocation / threading error occurs
    // it will return > 0 if sodium_init() was already called,
    // but this is ok/noop
    //
    // NO INVARIANTS
    unsafe {
        if rust_sodium_sys::sodium_init() > -1 {
            return Ok(());
        }
    }
    Err(CryptoError::InternalSodium)
}

pub(crate) fn randombytes_buf(buf: &mut [u8]) {
    // randombytes_buf doesn't return anything and only acts on an already
    // allocated buffer - there are no error conditions possible here.
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    unsafe {
        rust_sodium_sys::randombytes_buf(raw_ptr_void!(buf), buf.len());
    }
}

pub(crate) fn crypto_generichash(
    hash: &mut [u8],
    message: &[u8],
    key: Option<&[u8]>,
) -> CryptoResult<()> {
    if hash.len() < rust_sodium_sys::crypto_generichash_BYTES_MIN as usize
        || hash.len() > rust_sodium_sys::crypto_generichash_BYTES_MAX as usize
    {
        return Err(CryptoError::BadHashSize);
    }

    let (key_len, key) = match key {
        Some(key) => {
            if key.len() < rust_sodium_sys::crypto_generichash_KEYBYTES_MIN as usize
                || key.len() > rust_sodium_sys::crypto_generichash_KEYBYTES_MAX as usize
            {
                return Err(CryptoError::BadKeySize);
            }
            (key.len(), raw_ptr_char_immut!(key))
        }
        None => (0, std::ptr::null()),
    };

    // crypto_generichash can error on bad hash size / or bad key size
    // it can handle message sizes up to the max c_ulonglong.
    // we check sizes above for more detailed errors
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    //   - hash size - checked above
    //   - key size - checked above
    unsafe {
        if rust_sodium_sys::crypto_generichash(
            raw_ptr_char!(hash),
            hash.len(),
            raw_ptr_char_immut!(message),
            message.len() as libc::c_ulonglong,
            key,
            key_len,
        ) == 0 as libc::c_int
        {
            return Ok(());
        }
        Err(CryptoError::InternalSodium)
    }
}

pub(crate) fn crypto_sign_seed_keypair(
    pub_key: &mut [u8],
    sec_key: &mut [u8],
    seed: &[u8],
) -> CryptoResult<()> {
    if pub_key.len() != rust_sodium_sys::crypto_sign_PUBLICKEYBYTES as usize {
        return Err(CryptoError::BadPublicKeySize);
    }

    if sec_key.len() != rust_sodium_sys::crypto_sign_SECRETKEYBYTES as usize {
        return Err(CryptoError::BadSecretKeySize);
    }

    if seed.len() != rust_sodium_sys::crypto_sign_SEEDBYTES as usize {
        return Err(CryptoError::BadSeedSize);
    }

    // crypto_sign_seed_keypair mainly fails from sizes enforced above
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    //   - pub_key size - checked above
    //   - sec_key size - checked above
    //   - seed size - checked above
    unsafe {
        if rust_sodium_sys::crypto_sign_seed_keypair(
            raw_ptr_char!(pub_key),
            raw_ptr_char!(sec_key),
            raw_ptr_char_immut!(seed),
        ) == 0 as libc::c_int
        {
            return Ok(());
        }
        Err(CryptoError::InternalSodium)
    }
}

pub(crate) fn crypto_sign_keypair(pub_key: &mut [u8], sec_key: &mut [u8]) -> CryptoResult<()> {
    if pub_key.len() != rust_sodium_sys::crypto_sign_PUBLICKEYBYTES as usize {
        return Err(CryptoError::BadPublicKeySize);
    }

    if sec_key.len() != rust_sodium_sys::crypto_sign_SECRETKEYBYTES as usize {
        return Err(CryptoError::BadSecretKeySize);
    }

    // crypto_sign_seed_keypair mainly fails from sizes enforced above
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    //   - pub_key size - checked above
    //   - sec_key size - checked above
    unsafe {
        if rust_sodium_sys::crypto_sign_keypair(raw_ptr_char!(pub_key), raw_ptr_char!(sec_key))
            == 0 as libc::c_int
        {
            return Ok(());
        }
        Err(CryptoError::InternalSodium)
    }
}

pub(crate) fn crypto_sign_detached(
    signature: &mut [u8],
    message: &[u8],
    sec_key: &[u8],
) -> CryptoResult<()> {
    if signature.len() != rust_sodium_sys::crypto_sign_BYTES as usize {
        return Err(CryptoError::BadSignatureSize);
    }

    if sec_key.len() != rust_sodium_sys::crypto_sign_SECRETKEYBYTES as usize {
        return Err(CryptoError::BadSecretKeySize);
    }

    // crypto_sign_detached mainly failes from sized checked above
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    //   - signature size - checked above
    //   - sec_key size - checked above
    unsafe {
        if rust_sodium_sys::crypto_sign_detached(
            raw_ptr_char!(signature),
            std::ptr::null_mut(),
            raw_ptr_char_immut!(message),
            message.len() as libc::c_ulonglong,
            raw_ptr_char_immut!(sec_key),
        ) == 0 as libc::c_int
        {
            return Ok(());
        }
        Err(CryptoError::InternalSodium)
    }
}

pub(crate) fn crypto_sign_verify_detached(
    signature: &[u8],
    message: &[u8],
    pub_key: &[u8],
) -> CryptoResult<bool> {
    if signature.len() != rust_sodium_sys::crypto_sign_BYTES as usize {
        return Err(CryptoError::BadSignatureSize);
    }

    if pub_key.len() != rust_sodium_sys::crypto_sign_PUBLICKEYBYTES as usize {
        return Err(CryptoError::BadPublicKeySize);
    }

    // crypto_sign_verify_detached mainly failes from sized checked above
    //
    // INVARIANTS:
    //   - sodium_init() was called (enforced by plugin system)
    //   - signature size - checked above
    //   - pub_key size - checked above
    unsafe {
        Ok(rust_sodium_sys::crypto_sign_verify_detached(
            raw_ptr_char_immut!(signature),
            raw_ptr_char_immut!(message),
            message.len() as libc::c_ulonglong,
            raw_ptr_char_immut!(pub_key),
        ) == 0 as libc::c_int)
    }
}
