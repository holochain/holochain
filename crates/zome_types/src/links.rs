use holochain_serialized_bytes::prelude::*;

#[derive(
    Debug,
    Clone,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
)]
pub struct LinkTag(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl LinkTag {
    /// New tag from bytes
    pub fn new<T>(t: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self(t.into())
    }
}

impl From<Vec<u8>> for LinkTag {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}