use crate::*;

pub(crate) fn randombytes_buf(buf: &mut [u8]) {
    unsafe {
        // randombytes_buf doesn't return anything
        rust_sodium_sys::randombytes_buf(raw_ptr_void!(buf), buf.len());
    }
}

pub(crate) fn crypto_generichash(
    hash: &mut [u8],
    message: &[u8],
    key: Option<&[u8]>,
) -> CryptoResult<()> {
    unsafe {
        let (key_len, key) = match key {
            Some(key) => (key.len(), raw_ptr_char_immut!(key)),
            None => (0, std::ptr::null()),
        };
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
