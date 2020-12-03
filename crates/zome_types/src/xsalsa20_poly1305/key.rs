use holochain_serialized_bytes::prelude::*;

/// Secretbox keys are 32 bytes long.
pub const KEY_BYTES: usize = 32;

#[derive(Clone, Copy, SerializedBytes)]
pub struct XSalsa20Poly1305Key([u8; KEY_BYTES]);
pub type SecretBoxKey = XSalsa20Poly1305Key;

// Secretbox keys are definitely secrets.
crate::crypto_secret!(XSalsa20Poly1305Key, KEY_BYTES);
