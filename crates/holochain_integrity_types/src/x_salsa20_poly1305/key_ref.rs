use holochain_serialized_bytes::prelude::*;

/// Key refs represent shared secrets stored in the keystore.
/// They can either be user-specified, or auto-generated at time of
/// secret creation, or ingestion.
#[derive(Debug)]
pub struct KeyRef(Box<str>);

impl std::ops::Deref for KeyRef {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for KeyRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for KeyRef {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl PartialEq for KeyRef {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        self.0.as_bytes().ct_eq(&other.0.as_bytes()).into()
    }
}

impl Eq for KeyRef {}

impl From<&str> for KeyRef {
    #[inline(always)]
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<&String> for KeyRef {
    #[inline(always)]
    fn from(s: &String) -> Self {
        s.clone().into()
    }
}

impl From<String> for KeyRef {
    #[inline(always)]
    fn from(s: String) -> Self {
        Self(s.into_boxed_str())
    }
}

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
