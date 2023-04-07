use super::*;
use crate::{error::HoloHashError, AgentPubKey, EntryHash};
use std::convert::TryInto;

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

/// The AnyDht (composite) HashType
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize, SerializedBytes),
    serde(from = "EntrySerial", into = "EntrySerial")
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum Entry {
    /// The hash of an Agent entry
    Agent,
    /// The hash of any entry other than an Agent entry
    NonAgent,
}

impl HashType for Entry {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            Entry::NonAgent => NonAgentEntry::new().get_prefix(),
            Entry::Agent => Agent::new().get_prefix(),
        }
    }

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        match prefix {
            primitive::ENTRY_PREFIX => Ok(Entry::NonAgent),
            primitive::AGENT_PREFIX => Ok(Entry::Agent),
            _ => Err(HoloHashError::BadPrefix(
                "Entry".to_string(),
                prefix.try_into().expect("3 byte prefix"),
            )),
        }
    }

    fn hash_name(self) -> &'static str {
        "EntryHash"
    }
}

impl HashTypeAsync for Entry {}

#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize)
)]
enum EntrySerial {
    /// The hash of an Entry of EntryType::Agent
    Agent(Agent),
    /// The hash of any other EntryType
    NonAgentEntry(NonAgentEntry),
}

impl From<Entry> for EntrySerial {
    fn from(t: Entry) -> Self {
        match t {
            Entry::Agent => EntrySerial::Agent(Agent),
            Entry::NonAgent => EntrySerial::NonAgentEntry(NonAgentEntry),
        }
    }
}

impl From<EntrySerial> for Entry {
    fn from(t: EntrySerial) -> Self {
        match t {
            EntrySerial::Agent(_) => Entry::Agent,
            EntrySerial::NonAgentEntry(_) => Entry::NonAgent,
        }
    }
}

impl From<AgentPubKey> for EntryHash {
    fn from(hash: AgentPubKey) -> EntryHash {
        hash.retype(crate::hash_type::Entry::Agent)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for crate::HoloHash<Entry> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let entry = Entry::arbitrary(u)?;
        let some_hash = crate::HoloHash::<NonAgentEntry>::arbitrary(u)?;
        Ok(some_hash.retype(entry))
    }
}

/// The AnyDht (composite) HashType
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serialization",
    derive(serde::Deserialize, serde::Serialize, SerializedBytes),
    serde(from = "AnyDhtSerial", into = "AnyDhtSerial")
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AnyDht {
    /// The hash of an Entry
    Entry(Entry),
    /// The hash of an action
    Action,
}

impl HashType for AnyDht {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            AnyDht::Entry(entry) => entry.get_prefix(),
            AnyDht::Action => Action::new().get_prefix(),
        }
    }

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        match prefix {
            primitive::AGENT_PREFIX => Ok(AnyDht::Entry(Entry::Agent)),
            primitive::ENTRY_PREFIX => Ok(AnyDht::Entry(Entry::NonAgent)),
            primitive::ACTION_PREFIX => Ok(AnyDht::Action),
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
    Action(Action),
    /// The hash of any other EntryType
    Entry(Entry),
}

impl From<AnyDht> for AnyDhtSerial {
    fn from(t: AnyDht) -> Self {
        match t {
            AnyDht::Action => AnyDhtSerial::Action(Action),
            AnyDht::Entry(entry) => AnyDhtSerial::Entry(entry),
        }
    }
}

impl From<AnyDhtSerial> for AnyDht {
    fn from(t: AnyDhtSerial) -> Self {
        match t {
            AnyDhtSerial::Action(_) => AnyDht::Action,
            AnyDhtSerial::Entry(entry) => AnyDht::Entry(entry),
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
    Entry(Entry),
    /// The hash of an action
    Action,
    /// The hash of an External thing.
    External,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for crate::HoloHash<AnyLinkable> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let any_linkable = AnyLinkable::arbitrary(u)?;
        let some_hash = crate::HoloHash::<Entry>::arbitrary(u)?;
        Ok(some_hash.retype(any_linkable))
    }
}

impl HashType for AnyLinkable {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            Self::Entry(entry) => entry.get_prefix(),
            Self::Action => Action::new().get_prefix(),
            Self::External => External::new().get_prefix(),
        }
    }

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        match prefix {
            primitive::AGENT_PREFIX => Ok(AnyLinkable::Entry(Entry::Agent)),
            primitive::ENTRY_PREFIX => Ok(AnyLinkable::Entry(Entry::NonAgent)),
            primitive::ACTION_PREFIX => Ok(AnyLinkable::Action),
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
    Action(Action),
    /// The hash of any other EntryType
    Entry(Entry),
    /// The hash of any external thing.
    External(External),
}

impl From<AnyLinkable> for AnyLinkableSerial {
    fn from(t: AnyLinkable) -> Self {
        match t {
            AnyLinkable::Action => Self::Action(Action),
            AnyLinkable::Entry(entry) => Self::Entry(entry),
            AnyLinkable::External => Self::External(External),
        }
    }
}

impl From<AnyLinkableSerial> for AnyLinkable {
    fn from(t: AnyLinkableSerial) -> Self {
        match t {
            AnyLinkableSerial::Action(_) => Self::Action,
            AnyLinkableSerial::Entry(entry) => Self::Entry(entry),
            AnyLinkableSerial::External(_) => Self::External,
        }
    }
}

impl From<AnyDht> for AnyLinkable {
    fn from(t: AnyDht) -> Self {
        match t {
            AnyDht::Entry(entry) => AnyLinkable::Entry(entry),
            AnyDht::Action => AnyLinkable::Action,
        }
    }
}
