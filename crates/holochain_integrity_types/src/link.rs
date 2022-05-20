use std::borrow::Cow;

use holochain_serialized_bytes::prelude::*;

use crate::GlobalZomeTypeId;
#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct LinkType(pub u8);

impl LinkType {
    pub fn new(u: u8) -> Self {
        Self(u)
    }

    pub fn into_inner(self) -> u8 {
        self.0
    }
}

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LinkTypeName(pub Cow<'static, str>);

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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct LinkTag(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl LinkTag {
    /// New tag from bytes
    pub fn new<T>(t: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self(t.into())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for LinkTag {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

impl From<()> for LinkTag {
    fn from(_: ()) -> Self {
        Self(Vec::new())
    }
}

impl AsRef<Vec<u8>> for LinkTag {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}

impl std::ops::Deref for LinkType {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&'static str> for LinkTypeName {
    fn from(s: &'static str) -> Self {
        LinkTypeName(s.into())
    }
}

impl From<String> for LinkTypeName {
    fn from(s: String) -> Self {
        LinkTypeName(s.into())
    }
}

impl From<u8> for LinkType {
    fn from(t: u8) -> Self {
        Self(t)
    }
}

impl From<LinkType> for GlobalZomeTypeId {
    fn from(v: LinkType) -> Self {
        Self(v.0)
    }
}
