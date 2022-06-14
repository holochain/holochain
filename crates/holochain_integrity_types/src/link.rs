use crate::GlobalZomeTypeId;
use holochain_serialized_bytes::prelude::*;

/// 1kb limit on LinkTags.
/// Tags are used as keys to the database to allow
/// fast lookup so they should be small.
pub const MAX_TAG_SIZE: usize = 1000;

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

#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// A range of [`LinkType`] for quering links.
pub enum LinkTypeRange {
    /// Filter out all link types.
    /// This is only useful for combining with other ranges.
    Empty,
    /// Return links of any type.
    Full,
    /// Return links that are within the given range.
    Inclusive(core::ops::RangeInclusive<LinkType>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// A list of [`LinkTypeRanges`] for quering links.
pub struct LinkTypeRanges(pub Vec<LinkTypeRange>);

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

impl LinkTypeRange {
    /// Check if the link type is contained in this range.
    pub fn contains(&self, link_type: &LinkType) -> bool {
        match self {
            LinkTypeRange::Empty => false,
            LinkTypeRange::Full => true,
            LinkTypeRange::Inclusive(r) => r.contains(link_type),
        }
    }
}

impl LinkTypeRanges {
    /// Check if the link type is contained in any of these ranges.
    pub fn contains(&self, link_type: &LinkType) -> bool {
        self.0.iter().all(|r| !matches!(r, LinkTypeRange::Empty))
            && self.0.iter().any(|r| r.contains(link_type))
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

impl From<LinkType> for GlobalZomeTypeId {
    fn from(v: LinkType) -> Self {
        Self(v.0)
    }
}

impl From<GlobalZomeTypeId> for LinkType {
    fn from(v: GlobalZomeTypeId) -> Self {
        Self(v.0)
    }
}

impl From<core::ops::Range<LinkType>> for LinkTypeRange {
    fn from(r: core::ops::Range<LinkType>) -> Self {
        if r.is_empty() {
            Self::Empty
        } else {
            // Safe to convert to inclusive because it's not empty
            Self::Inclusive(core::ops::RangeInclusive::new(
                r.start,
                LinkType(r.end.0 - 1),
            ))
        }
    }
}

impl From<core::ops::RangeFull> for LinkTypeRange {
    fn from(_: core::ops::RangeFull) -> Self {
        Self::Full
    }
}

impl From<core::ops::RangeFull> for LinkTypeRanges {
    fn from(_: core::ops::RangeFull) -> Self {
        Self(vec![LinkTypeRange::Full])
    }
}

impl From<core::ops::RangeInclusive<LinkType>> for LinkTypeRange {
    fn from(r: core::ops::RangeInclusive<LinkType>) -> Self {
        if r.is_empty() {
            Self::Empty
        } else if *r.start() == LinkType(0) && *r.end() == LinkType(u8::MAX) {
            Self::Full
        } else {
            Self::Inclusive(r)
        }
    }
}

impl From<core::ops::RangeTo<LinkType>> for LinkTypeRange {
    fn from(r: core::ops::RangeTo<LinkType>) -> Self {
        if r.end.0 == 0 {
            Self::Empty
        } else {
            // Safe to subtract 1 because it's not empty.
            Self::Inclusive(LinkType(0)..=LinkType(r.end.0 - 1))
        }
    }
}

impl From<core::ops::RangeToInclusive<LinkType>> for LinkTypeRange {
    fn from(r: core::ops::RangeToInclusive<LinkType>) -> Self {
        if r.end.0 == 0 {
            Self::Empty
        } else if r.end.0 == u8::MAX {
            Self::Full
        } else {
            Self::Inclusive(LinkType(0)..=r.end)
        }
    }
}

impl From<core::ops::RangeFrom<LinkType>> for LinkTypeRange {
    fn from(r: core::ops::RangeFrom<LinkType>) -> Self {
        if r.start.0 == 0 {
            Self::Full
        } else {
            Self::Inclusive(r.start..=LinkType(u8::MAX))
        }
    }
}

impl From<LinkType> for LinkTypeRange {
    fn from(t: LinkType) -> Self {
        Self::Inclusive(t..=t)
    }
}

impl From<LinkType> for LinkTypeRanges {
    fn from(t: LinkType) -> Self {
        Self(vec![t.into()])
    }
}

impl From<LinkTypeRange> for LinkTypeRanges {
    fn from(r: LinkTypeRange) -> Self {
        LinkTypeRanges(vec![r])
    }
}

impl<E> TryFrom<Box<dyn FnOnce() -> Result<LinkTypeRanges, E>>> for LinkTypeRanges {
    type Error = E;

    fn try_from(f: Box<dyn FnOnce() -> Result<LinkTypeRanges, E>>) -> Result<Self, Self::Error> {
        f()
    }
}

impl<E> TryFrom<Box<dyn FnOnce() -> Result<LinkTypeRange, E>>> for LinkTypeRanges {
    type Error = E;

    fn try_from(f: Box<dyn FnOnce() -> Result<LinkTypeRange, E>>) -> Result<Self, Self::Error> {
        f().map(Self::from)
    }
}

impl<T, E> TryFrom<Vec<T>> for LinkTypeRanges
where
    T: TryInto<LinkTypeRange, Error = E>,
{
    type Error = E;

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        value
            .into_iter()
            .map(TryInto::<LinkTypeRange>::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

impl<T, E, const N: usize> TryFrom<[T; N]> for LinkTypeRanges
where
    T: TryInto<LinkTypeRange, Error = E>,
{
    type Error = E;

    fn try_from(value: [T; N]) -> Result<Self, Self::Error> {
        value
            .into_iter()
            .map(TryInto::<LinkTypeRange>::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

impl<T, E, const N: usize> TryFrom<&[T; N]> for LinkTypeRanges
where
    LinkTypeRange: for<'a> TryFrom<&'a T, Error = E>,
{
    type Error = E;

    fn try_from(value: &[T; N]) -> Result<Self, Self::Error> {
        value
            .iter()
            .map(TryInto::<LinkTypeRange>::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

impl<'a, T, E> TryFrom<&'a [T]> for LinkTypeRanges
where
    &'a T: TryInto<LinkTypeRange, Error = E>,
{
    type Error = E;

    fn try_from(value: &'a [T]) -> Result<Self, Self::Error> {
        value
            .iter()
            .map(TryInto::<LinkTypeRange>::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}
