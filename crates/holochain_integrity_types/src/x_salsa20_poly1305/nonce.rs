use holochain_secure_primitive::secure_primitive;
use holochain_serialized_bytes::prelude::*;
use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

pub const NONCE_BYTES: usize = 24;

#[derive(Clone, Copy, SerializedBytes, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub struct XSalsa20Poly1305Nonce([u8; NONCE_BYTES]);
pub type SecretBoxNonce = XSalsa20Poly1305Nonce;

// A nonce is public but it does need to inherit all the fixed array serialization and in the
// future it will be useful to have generation from random bytes as it MUST be UNIQUE.
// Currently lair does the nonce generation for us.
secure_primitive!(XSalsa20Poly1305Nonce, NONCE_BYTES);
