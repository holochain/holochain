use crate::ZomeIndex;
use holochain_serialized_bytes::prelude::*;

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

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
/// Filter on a set of [`LinkType`]s.
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
