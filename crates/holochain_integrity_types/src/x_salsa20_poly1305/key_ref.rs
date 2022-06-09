use holochain_serialized_bytes::prelude::*;
use std::sync::Arc;

/// Key refs represent shared secrets stored in the keystore.
/// They can either be user-specified, or auto-generated at time of
/// secret creation, or ingestion.
#[derive(Debug, Clone, SerializedBytes)]
pub struct XSalsa20Poly1305KeyRef(Arc<[u8]>);

impl serde::Serialize for XSalsa20Poly1305KeyRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for XSalsa20Poly1305KeyRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        Ok(Self(inner.into_boxed_slice().into()))
    }
}

impl std::ops::Deref for XSalsa20Poly1305KeyRef {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for XSalsa20Poly1305KeyRef {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::borrow::Borrow<[u8]> for XSalsa20Poly1305KeyRef {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for XSalsa20Poly1305KeyRef {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for XSalsa20Poly1305KeyRef {}

impl From<&[u8]> for XSalsa20Poly1305KeyRef {
    #[inline(always)]
    fn from(b: &[u8]) -> Self {
        b.to_vec().into()
    }
}

impl<const N: usize> From<[u8; N]> for XSalsa20Poly1305KeyRef {
    #[inline(always)]
    fn from(b: [u8; N]) -> Self {
        b.to_vec().into()
    }
}

impl From<&Vec<u8>> for XSalsa20Poly1305KeyRef {
    #[inline(always)]
    fn from(b: &Vec<u8>) -> Self {
        b.clone().into()
    }
}

impl From<Vec<u8>> for XSalsa20Poly1305KeyRef {
    #[inline(always)]
    fn from(b: Vec<u8>) -> Self {
        b.into_boxed_slice().into()
    }
}

impl From<Box<[u8]>> for XSalsa20Poly1305KeyRef {
    #[inline(always)]
    fn from(b: Box<[u8]>) -> Self {
        Self(b.into())
    }
}
