use holochain_secure_primitive::secure_primitive;
use holochain_serialized_bytes::prelude::*;

pub const ED25519_PUB_KEY_BYTES: usize = 32;

#[derive(Clone, Copy, SerializedBytes)]
pub struct Ed25519PubKey([u8; ED25519_PUB_KEY_BYTES]);

secure_primitive!(Ed25519PubKey, ED25519_PUB_KEY_BYTES);
