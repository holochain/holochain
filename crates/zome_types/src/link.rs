use holochain_serialized_bytes::prelude::*;

/// Opaque tag for the link applied at the app layer, used to differentiate
/// between different semantics and validation rules for different links
#[derive(
    Debug,
    PartialOrd,
    Ord,
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

impl AsRef<Vec<u8>> for LinkTag {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}

#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
)]
pub struct Link {
    /// The [Entry] being linked to
    pub target: holo_hash_core::HoloHashCore,
    /// When the link was added
    pub timestamp: std::time::SystemTime,
    /// A tag used to find this link
    pub tag: LinkTag,
}

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Clone, Debug)]
pub struct Links(Vec<Link>);

impl From<Vec<Link>> for Links {
    fn from(v: Vec<Link>) -> Self {
        Self(v)
    }
}

impl From<Links> for Vec<Link> {
    fn from(links: Links) -> Self {
        links.0
    }
}

impl Links {
    pub fn into_inner(self) -> Vec<Link> {
        self.into()
    }
}
