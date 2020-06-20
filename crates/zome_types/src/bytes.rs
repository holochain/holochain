//! represent arbitrary bytes (not serialized)
//! e.g. totally random crypto bytes from random_bytes

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[repr(transparent)]
/// a transparent newtype to wrap Vec<u8> so we can name a vector of bytes "Bytes" in the compiler
pub struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
