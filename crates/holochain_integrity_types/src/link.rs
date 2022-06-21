use crate::LocalZomeTypeId;
use crate::ZomeId;
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
    Types(Vec<(ZomeId, Vec<LinkType>)>),
    /// Return links that match any types defined
    /// in any of this zomes dependencies.
    Dependencies(Vec<ZomeId>),
}

impl LinkTypeFilter {
    pub fn zome_for<E>(link_type: impl TryInto<ZomeId, Error = E>) -> Result<Self, E> {
        link_type.try_into().map(LinkTypeFilter::single_dep)
    }

    pub fn contains(&self, zome_id: &ZomeId, link_type: &LinkType) -> bool {
        match self {
            LinkTypeFilter::Types(types) => types
                .iter()
                .any(|(z, types)| z == zome_id && types.contains(link_type)),
            LinkTypeFilter::Dependencies(deps) => deps.contains(zome_id),
        }
    }

    pub fn single_type(zome_id: ZomeId, link_type: LinkType) -> Self {
        Self::Types(vec![(zome_id, vec![link_type])])
    }

    pub fn single_dep(zome_id: ZomeId) -> Self {
        Self::Dependencies(vec![zome_id])
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

impl From<LinkType> for LocalZomeTypeId {
    fn from(v: LinkType) -> Self {
        Self(v.0)
    }
}

impl From<LocalZomeTypeId> for LinkType {
    fn from(v: LocalZomeTypeId) -> Self {
        Self(v.0)
    }
}

impl From<(ZomeId, LinkType)> for LinkType {
    fn from((_, l): (ZomeId, LinkType)) -> Self {
        l
    }
}

impl From<(ZomeId, LinkType)> for ZomeId {
    fn from((z, _): (ZomeId, LinkType)) -> Self {
        z
    }
}
