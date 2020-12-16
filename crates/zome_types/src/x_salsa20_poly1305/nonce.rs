use holochain_serialized_bytes::prelude::*;

pub const NONCE_BYTES: usize = 24;

#[derive(Clone, Copy, SerializedBytes)]
pub struct XSalsa20Poly1305Nonce([u8; NONCE_BYTES]);
pub type SecretBoxNonce = XSalsa20Poly1305Nonce;

// A nonce is technically public but it does need to inherit all the fixed array serialization and
// generation from random bytes as it MUST be UNIQUE.
crate::crypto_secret!(XSalsa20Poly1305Nonce, NONCE_BYTES);
