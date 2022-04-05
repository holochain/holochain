use holochain_serialized_bytes::prelude::*;

/// Key refs are the same length as the keys themselves.
/// The key ref is just a sha256 of the key. There are no benefits, only downsides, to having
/// either a larger or smaller set of outputs (ref size) vs. the set of inputs (key size).
pub const KEY_REF_BYTES: usize = 32;

#[derive(Clone, Copy, SerializedBytes)]
pub struct XSalsa20Poly1305KeyRef([u8; KEY_REF_BYTES]);
pub type SecretBoxKeyRef = XSalsa20Poly1305KeyRef;

// Key refs need to be exactly the length of the key ref bytes hash, not doing so could cause
// problems.
crate::secure_primitive!(XSalsa20Poly1305KeyRef, KEY_REF_BYTES);
