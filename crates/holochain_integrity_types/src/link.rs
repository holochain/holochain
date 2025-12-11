use crate::ZomeIndex;
use holochain_serialized_bytes::prelude::*;
use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

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
    TS,
)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub struct LinkType(pub u8);

impl LinkType {
    pub fn new(u: u8) -> Self {
        Self(u)
    }
    pub fn into_inner(self) -> u8 {
        self.0
    }
}

/// Opaque tag for the link applied at the app layer, used to differentiate
/// between different semantics and validation rules for different links
#[derive(
    Debug, PartialOrd, Ord, Clone, Hash, serde::Serialize, serde::Deserialize, PartialEq, Eq, TS,
)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
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

/// Filter on a set of [`LinkType`]s.
#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub enum LinkTypeFilter {
    /// Return links that match any of these types.
    Types(Vec<(ZomeIndex, Vec<LinkType>)>),
    /// Return links that match any types defined
    /// in any of this zomes dependencies.
    Dependencies(Vec<ZomeIndex>),
}

/// A helper trait for finding the app defined link type
/// from a [`ZomeIndex`] and [`LinkType`].
///
/// If the zome id is a dependency of the calling zome and
/// the link type is out of range (greater than the number of defined
/// link types) then a guest error is return (which will invalidate an op
/// if used in the validation callback).
///
/// If the zome id is **not** a dependency of the calling zome then
/// this will return [`None`].
pub trait LinkTypesHelper: Sized {
    /// The error associated with this conversion.
    type Error;
    /// Check if the [`ZomeIndex`] and [`LinkType`] matches one of the
    /// `ZomeLinkTypeKey::from(Self::variant)` and if
    /// it does return that type.
    fn from_type<Z, I>(zome_index: Z, link_type: I) -> Result<Option<Self>, Self::Error>
    where
        Z: Into<ZomeIndex>,
        I: Into<LinkType>;
}

impl LinkTypesHelper for () {
    type Error = core::convert::Infallible;

    fn from_type<Z, I>(_zome_index: Z, _link_type: I) -> Result<Option<Self>, Self::Error>
    where
        Z: Into<ZomeIndex>,
        I: Into<LinkType>,
    {
        Ok(Some(()))
    }
}

impl LinkTypeFilter {
    pub fn zome_for<E>(link_type: impl TryInto<ZomeIndex, Error = E>) -> Result<Self, E> {
        link_type.try_into().map(LinkTypeFilter::single_dep)
    }

    pub fn contains(&self, zome_index: &ZomeIndex, link_type: &LinkType) -> bool {
        match self {
            LinkTypeFilter::Types(types) => types
                .iter()
                .any(|(z, types)| z == zome_index && types.contains(link_type)),
            LinkTypeFilter::Dependencies(deps) => deps.contains(zome_index),
        }
    }

    pub fn single_type(zome_index: ZomeIndex, link_type: LinkType) -> Self {
        Self::Types(vec![(zome_index, vec![link_type])])
    }

    pub fn single_dep(zome_index: ZomeIndex) -> Self {
        Self::Dependencies(vec![zome_index])
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

impl From<u8> for LinkType {
    fn from(t: u8) -> Self {
        Self(t)
    }
}

impl From<(ZomeIndex, LinkType)> for LinkType {
    fn from((_, l): (ZomeIndex, LinkType)) -> Self {
        l
    }
}

impl From<(ZomeIndex, LinkType)> for ZomeIndex {
    fn from((z, _): (ZomeIndex, LinkType)) -> Self {
        z
    }
}

impl From<String> for LinkTag {
    fn from(s: String) -> Self {
        Self(s.into_bytes())
    }
}

impl From<&str> for LinkTag {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

impl TryInto<String> for LinkTag {
    type Error = std::string::FromUtf8Error;

    fn try_into(self) -> Result<String, Self::Error> {
        String::from_utf8(self.0)
    }
}

/// Convert a `LinkTag` into `SerializedBytes` (Infallible)
impl From<LinkTag> for SerializedBytes {
    fn from(tag: LinkTag) -> SerializedBytes {
        SerializedBytes::from(UnsafeBytes::from(tag.0))
    }
}

/// Convert `SerializedBytes` into a `LinkTag` (Infallible)
impl From<SerializedBytes> for LinkTag {
    fn from(sb: SerializedBytes) -> Self {
        Self::new(sb.bytes().clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
    pub struct Data {
        pub latitude: f64,
        pub longitude: f64,
    }

    #[test]
    fn link_tag_roundtrip() {
        let location = Data {
            latitude: 4.518758758758,
            longitude: 4.718758758973,
        };
        let sb = SerializedBytes::try_from(location.clone()).unwrap();
        let tag = LinkTag::from(sb);
        let back_to_sb: SerializedBytes = tag.into();
        let back_to_location: Data = back_to_sb.try_into().unwrap();
        assert_eq!(location, back_to_location);
    }
}
