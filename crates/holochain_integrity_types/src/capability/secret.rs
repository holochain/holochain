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
pub struct CapSecret(pub CapSecretBytes);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for CapSecret {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut buf = [0; CAP_SECRET_BYTES];
        u.fill_buffer(&mut buf)?;
        Ok(CapSecret(buf))
    }
}

// Capability secrets are not cryptographic secrets.
// They aren't used in any cryptographic algorithm.
// They are closer to API keys in that they may provide access to specific functions on a specific
// device if it is accepting incoming connections. Still secret but there are mitigating factors
// such as the ability to revoke a secret, and to assign it to specific recipients ahead of time
// if they are a known closed set.
crate::secure_primitive!(CapSecret, CAP_SECRET_BYTES);
