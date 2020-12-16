use holochain_serialized_bytes::prelude::*;

/// The number of bits we want for a comfy secret.
pub const CAP_SECRET_BITS: usize = 512;
/// The number of bytes we want for a comfy secret.
pub const CAP_SECRET_BYTES: usize = CAP_SECRET_BITS / 8;
/// A fixed size array of bytes that a secret must be.
pub type CapSecretBytes = [u8; CAP_SECRET_BYTES];

/// A CapSecret is used by a caller to prove to a callee access to a committed CapGrant.
///
/// It is a random, unique identifier for the capability, which is shared by
/// the grantor to allow access to others. The grantor can optionally further restrict usage of the
/// secret to specific agents.
///
/// @todo enforce that secrets are unique across all grants in a chain.
#[derive(Clone, Copy, SerializedBytes)]
pub struct CapSecret(CapSecretBytes);

crate::crypto_secret!(CapSecret, CAP_SECRET_BYTES);
