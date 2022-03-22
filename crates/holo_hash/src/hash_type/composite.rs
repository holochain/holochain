use super::*;
use crate::error::HoloHashError;
use crate::HoloHash;
use std::convert::TryInto;

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

/// The AnyDht (composite) HashType
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize, SerializedBytes),
    serde(from = "AnyDhtSerial", into = "AnyDhtSerial")
)]
pub enum AnyDht {
    /// The hash of an Entry
    Entry,
    /// The hash of a Header
    Header,
}

impl HashType for AnyDht {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            AnyDht::Entry => Entry::new().get_prefix(),
            AnyDht::Header => Header::new().get_prefix(),
        }
    }

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        match prefix {
            primitive::ENTRY_PREFIX => Ok(AnyDht::Entry),
            primitive::HEADER_PREFIX => Ok(AnyDht::Header),
            _ => Err(HoloHashError::BadPrefix(
                "AnyDht".to_string(),
                prefix.try_into().expect("3 byte prefix"),
            )),
        }
    }

    fn hash_name(self) -> &'static str {
        "AnyDhtHash"
    }
}

impl HashTypeAsync for AnyDht {}

#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize)
)]
enum AnyDhtSerial {
    /// The hash of an Entry of EntryType::Agent
    Header(Header),
    /// The hash of any other EntryType
    Entry(Entry),
}

impl From<AnyDht> for AnyDhtSerial {
    fn from(t: AnyDht) -> Self {
        match t {
            AnyDht::Header => AnyDhtSerial::Header(Header),
            AnyDht::Entry => AnyDhtSerial::Entry(Entry),
        }
    }
}

impl From<AnyDhtSerial> for AnyDht {
    fn from(t: AnyDhtSerial) -> Self {
        match t {
            AnyDhtSerial::Header(_) => AnyDht::Header,
            AnyDhtSerial::Entry(_) => AnyDht::Entry,
        }
    }
}

/// The AnyLinkable (composite) HashType
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize, SerializedBytes),
    serde(from = "AnyLinkableSerial", into = "AnyLinkableSerial")
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AnyLinkable {
    /// The hash of an Entry
    Entry,
    /// The hash of a Header
    Header,
    /// The hash of an External thing.
    External,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for HoloHash<AnyLinkable> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let any_linkable = AnyLinkable::arbitrary(u)?;
        let some_hash = HoloHash::<Entry>::arbitrary(u)?;
        Ok(some_hash.retype(any_linkable))
    }
}

impl HashType for AnyLinkable {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            Self::Entry => Entry::new().get_prefix(),
            Self::Header => Header::new().get_prefix(),
            Self::External => External::new().get_prefix(),
        }
    }

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        match prefix {
            primitive::ENTRY_PREFIX => Ok(AnyLinkable::Entry),
            primitive::HEADER_PREFIX => Ok(AnyLinkable::Header),
            primitive::EXTERNAL_PREFIX => Ok(AnyLinkable::External),
            _ => Err(HoloHashError::BadPrefix(
                "AnyLinkable".to_string(),
                prefix.try_into().expect("3 byte prefix"),
            )),
        }
    }

    fn hash_name(self) -> &'static str {
        "AnyLinkableHash"
    }
}

impl HashTypeSync for AnyLinkable {}

#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize)
)]
enum AnyLinkableSerial {
    /// The hash of an Entry of EntryType::Agent
    Header(Header),
    /// The hash of any other EntryType
    Entry(Entry),
    /// The hash of any external thing.
    External(External),
}

impl From<AnyLinkable> for AnyLinkableSerial {
    fn from(t: AnyLinkable) -> Self {
        match t {
            AnyLinkable::Header => Self::Header(Header),
            AnyLinkable::Entry => Self::Entry(Entry),
            AnyLinkable::External => Self::External(External),
        }
    }
}

impl From<AnyLinkableSerial> for AnyLinkable {
    fn from(t: AnyLinkableSerial) -> Self {
        match t {
            AnyLinkableSerial::Header(_) => Self::Header,
            AnyLinkableSerial::Entry(_) => Self::Entry,
            AnyLinkableSerial::External(_) => Self::External,
        }
    }
}
