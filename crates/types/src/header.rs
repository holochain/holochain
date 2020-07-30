//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::{
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    prelude::*,
};
use conversions::WrongHeaderError;
use holo_hash::EntryHash;
use holochain_zome_types::entry_def::EntryVisibility;
pub use holochain_zome_types::header::HeaderHashed;
use holochain_zome_types::{
    element::{Element, SignedHeader},
    header::*,
    Entry,
};

use error::*;

pub mod error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
/// A header of one of the two types that create a new entry.
pub enum NewEntryHeader {
    /// A header which simply creates a new entry
    Create(EntryCreate),
    /// A header which creates a new entry that is semantically related to a
    /// previously created entry or header
    Update(EntryUpdate),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
/// A header of one of the two types that create a new entry.
pub enum WireNewEntryHeader {
    Create(WireEntryCreate),
    Update(WireEntryUpdate),
}

/// The minimum unique data for new entry header
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireEntryCreate {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    pub signature: Signature,
}

/// The minimum unique data for new entry header
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireEntryUpdate {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    pub intended_for: IntendedFor,
    pub replaces_address: HeaderHash,
    pub signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireElementDelete {
    pub delete: ElementDelete,
    pub signature: Signature,
}

impl NewEntryHeader {
    /// Get the entry on this header
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryHeader::Create(EntryCreate { entry_hash, .. })
            | NewEntryHeader::Update(EntryUpdate { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the visibility of this header
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryHeader::Create(EntryCreate { entry_type, .. })
            | NewEntryHeader::Update(EntryUpdate { entry_type, .. }) => entry_type.visibility(),
        }
    }
}

impl From<NewEntryHeader> for Header {
    fn from(h: NewEntryHeader) -> Self {
        match h {
            NewEntryHeader::Create(h) => Header::EntryCreate(h),
            NewEntryHeader::Update(h) => Header::EntryUpdate(h),
        }
    }
}

impl From<(EntryCreate, Signature)> for WireEntryCreate {
    fn from((ec, signature): (EntryCreate, Signature)) -> Self {
        Self {
            timestamp: ec.timestamp,
            author: ec.author,
            header_seq: ec.header_seq,
            prev_header: ec.prev_header,
            signature,
        }
    }
}

impl From<(EntryUpdate, Signature)> for WireEntryUpdate {
    fn from((eu, signature): (EntryUpdate, Signature)) -> Self {
        Self {
            timestamp: eu.timestamp,
            author: eu.author,
            header_seq: eu.header_seq,
            prev_header: eu.prev_header,
            intended_for: eu.intended_for,
            replaces_address: eu.replaces_address,
            signature,
        }
    }
}

impl WireElementDelete {
    pub async fn into_element(self) -> Element {
        Element::new(
            SignedHeaderHashed::from_content(SignedHeader(self.delete.into(), self.signature))
                .await,
            None,
        )
    }
}

impl TryFrom<SignedHeaderHashed> for WireElementDelete {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        Ok(Self {
            delete: h.into_content().try_into()?,
            signature,
        })
    }
}

impl WireNewEntryHeader {
    pub async fn into_element(self, entry_type: EntryType, entry: Entry) -> Element {
        let entry_hash = EntryHash::with_data(&entry).await;
        match self {
            WireNewEntryHeader::Create(ec) => {
                let signature = ec.signature;
                let ec = EntryCreate {
                    author: ec.author,
                    timestamp: ec.timestamp,
                    header_seq: ec.header_seq,
                    prev_header: ec.prev_header,
                    entry_type,
                    entry_hash,
                };
                Element::new(
                    SignedHeaderHashed::from_content(SignedHeader(ec.into(), signature)).await,
                    Some(entry),
                )
            }
            WireNewEntryHeader::Update(eu) => {
                let signature = eu.signature;
                let eu = EntryUpdate {
                    author: eu.author,
                    timestamp: eu.timestamp,
                    header_seq: eu.header_seq,
                    prev_header: eu.prev_header,
                    intended_for: eu.intended_for,
                    replaces_address: eu.replaces_address,
                    entry_type,
                    entry_hash,
                };
                Element::new(
                    SignedHeaderHashed::from_content(SignedHeader(eu.into(), signature)).await,
                    Some(entry),
                )
            }
        }
    }
}

impl TryFrom<SignedHeaderHashed> for WireNewEntryHeader {
    type Error = HeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (sh, _) = shh.into_inner();
        let (header, s) = sh.into();
        match header {
            Header::EntryCreate(ec) => Ok(Self::Create((ec, s).into())),
            Header::EntryUpdate(eu) => Ok(Self::Update((eu, s).into())),
            _ => return Err(HeaderError::NotNewEntry),
        }
    }
}

impl TryFrom<Header> for NewEntryHeader {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::EntryCreate(h) => Ok(NewEntryHeader::Create(h)),
            Header::EntryUpdate(h) => Ok(NewEntryHeader::Update(h)),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fixt::HeaderBuilderCommonFixturator,
        test_utils::{fake_dna_hash, fake_entry_content_hash},
    };
    use ::fixt::prelude::*;

    #[test]
    fn test_header_msgpack_roundtrip() {
        let orig: Header = Dna::from_builder(
            fake_dna_hash(1),
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
        )
        .into();
        let bytes = rmp_serde::to_vec_named(&orig).unwrap();
        let res: Header = rmp_serde::from_read_ref(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_entrycreate_msgpack_roundtrip() {
        let orig: Header = EntryCreate::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_content_hash(1).into(),
        )
        .into();
        let bytes = rmp_serde::to_vec_named(&orig).unwrap();
        println!("{:?}", bytes);
        let res: Header = rmp_serde::from_read_ref(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_entrycreate_serializedbytes_roundtrip() {
        let orig: Header = EntryCreate::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_content_hash(1).into(),
        )
        .into();
        let bytes: SerializedBytes = orig.clone().try_into().unwrap();
        let res: Header = bytes.try_into().unwrap();
        assert_eq!(orig, res);
    }
}
