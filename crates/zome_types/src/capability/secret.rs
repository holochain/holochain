use holochain_serialized_bytes::prelude::*;
use serde::de::Error;
use subtle::ConstantTimeEq;

/// The number of bits we want for a comfy secret.
pub const CAP_SECRET_BITS: usize = 512;
/// The number of bytes we want for a comfy secret.
pub const CAP_SECRET_BYTES: usize = CAP_SECRET_BITS / 8;
/// A fixed size array of bytes that a secret must be.
pub type CapSecretBytes = [u8; CAP_SECRET_BYTES];

/// A CapSecret is used to claim ability to exercise a capability.
///
/// It is a random, unique identifier for the capability, which is shared by
/// the Grantor to allow access to others.
/// A capability CAN be updated (replaced with a new one) with the same secret.
#[derive(Clone, Copy, SerializedBytes)]
pub struct CapSecret(CapSecretBytes);

impl serde::ser::Serialize for CapSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::de::Deserialize<'de> for CapSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let bytes: &[u8] = serde::de::Deserialize::deserialize(deserializer)?;
        if bytes.len() != CAP_SECRET_BYTES {
            return Err(D::Error::invalid_value(
                serde::de::Unexpected::Bytes(bytes),
                &"incorrect length cap secret",
            ));
        }
        let mut inner: [u8; CAP_SECRET_BYTES] = [0; CAP_SECRET_BYTES];
        inner.clone_from_slice(bytes);
        Ok(Self(inner))
    }
}

impl From<[u8; CAP_SECRET_BYTES]> for CapSecret {
    fn from(b: [u8; CAP_SECRET_BYTES]) -> Self {
        Self(b)
    }
}

impl PartialEq for CapSecret {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for CapSecret {}

impl std::fmt::Debug for CapSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0.to_vec(), f)
    }
}

impl From<&[u8]> for CapSecret {
    fn from(bytes: &[u8]) -> Self {
        let mut inner = [0; CAP_SECRET_BYTES];
        inner.copy_from_slice(bytes);
        inner.into()
    }
}

impl From<serde_bytes::ByteBuf> for CapSecret {
    fn from(byte_buf: serde_bytes::ByteBuf) -> Self {
        byte_buf.as_ref().into()
    }
}
