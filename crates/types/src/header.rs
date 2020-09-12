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
use derive_more::From;
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
    Create(CreateEntry),
    /// A header which creates a new entry that is semantically related to a
    /// previously created entry or header
    Update(UpdateEntry),
}

#[allow(missing_docs)]
#[derive(Debug, From)]
/// Same as NewEntryHeader but takes headers as reference
pub enum NewEntryHeaderRef<'a> {
    Create(&'a CreateEntry),
    Update(&'a UpdateEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
/// A header of one of the two types that create a new entry.
pub enum WireNewEntryHeader {
    Create(WireCreateEntry),
    Update(WireUpdateEntry),
}

/// The minimum unique data for CreateEntry headers
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireCreateEntry {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    pub signature: Signature,
}

/// The minimum unique data for UpdateEntry headers
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireUpdateEntry {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    pub original_entry_address: EntryHash,
    pub original_header_address: HeaderHash,
    pub signature: Signature,
}

/// This type is used when sending updates from the
/// original entry authority to someone asking for
/// metadata on that original entry.
/// ## How updates work
/// `UpdateEntry` headers create both a new entry and
/// a metadata relationship on the original entry.
/// This wire data represents the metadata relationship
/// which is stored on the original entry, i.e. this represents
/// the "forward" reference from the original entry to the new entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireUpdateEntryRelationship {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    /// Address of the original entry header
    pub original_header_address: HeaderHash,
    /// The entry that this update created
    pub new_entry_address: EntryHash,
    /// The entry type of the entry that this header created
    pub new_entry_type: EntryType,
    pub signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireDeleteElement {
    pub delete: DeleteElement,
    pub signature: Signature,
}

impl NewEntryHeader {
    /// Get the entry on this header
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryHeader::Create(CreateEntry { entry_hash, .. })
            | NewEntryHeader::Update(UpdateEntry { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the visibility of this header
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryHeader::Create(CreateEntry { entry_type, .. })
            | NewEntryHeader::Update(UpdateEntry { entry_type, .. }) => entry_type.visibility(),
        }
    }

    /// Get the timestamp of this header
    pub fn timestamp(&self) -> &holochain_zome_types::timestamp::Timestamp {
        match self {
            NewEntryHeader::Create(CreateEntry { timestamp, .. })
            | NewEntryHeader::Update(UpdateEntry { timestamp, .. }) => timestamp,
        }
    }
}

impl From<NewEntryHeader> for Header {
    fn from(h: NewEntryHeader) -> Self {
        match h {
            NewEntryHeader::Create(h) => Header::CreateEntry(h),
            NewEntryHeader::Update(h) => Header::UpdateEntry(h),
        }
    }
}

impl From<(CreateEntry, Signature)> for WireCreateEntry {
    fn from((ec, signature): (CreateEntry, Signature)) -> Self {
        Self {
            timestamp: ec.timestamp,
            author: ec.author,
            header_seq: ec.header_seq,
            prev_header: ec.prev_header,
            signature,
        }
    }
}

impl From<(UpdateEntry, Signature)> for WireUpdateEntry {
    fn from((eu, signature): (UpdateEntry, Signature)) -> Self {
        Self {
            timestamp: eu.timestamp,
            author: eu.author,
            header_seq: eu.header_seq,
            prev_header: eu.prev_header,
            original_entry_address: eu.original_entry_address,
            original_header_address: eu.original_header_address,
            signature,
        }
    }
}

impl WireDeleteElement {
    pub async fn into_element(self) -> Element {
        Element::new(
            SignedHeaderHashed::from_content_sync(SignedHeader(self.delete.into(), self.signature)),
            None,
        )
    }
}

impl WireUpdateEntryRelationship {
    /// Recreate the UpdateEntry Element without an Entry.
    /// Useful for creating dht ops
    pub async fn into_element(self, original_entry_address: EntryHash) -> Element {
        let eu = UpdateEntry {
            author: self.author,
            timestamp: self.timestamp,
            header_seq: self.header_seq,
            prev_header: self.prev_header,
            original_header_address: self.original_header_address,
            original_entry_address,
            entry_type: self.new_entry_type,
            entry_hash: self.new_entry_address,
        };
        Element::new(
            SignedHeaderHashed::from_content_sync(SignedHeader(
                Header::UpdateEntry(eu),
                self.signature,
            )),
            None,
        )
    }
}

impl NewEntryHeaderRef<'_> {
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryHeaderRef::Create(CreateEntry { entry_type, .. })
            | NewEntryHeaderRef::Update(UpdateEntry { entry_type, .. }) => entry_type,
        }
    }
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            NewEntryHeaderRef::Create(CreateEntry { entry_hash, .. })
            | NewEntryHeaderRef::Update(UpdateEntry { entry_hash, .. }) => entry_hash,
        }
    }
}

impl TryFrom<SignedHeaderHashed> for WireDeleteElement {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        Ok(Self {
            delete: h.into_content().try_into()?,
            signature,
        })
    }
}

impl TryFrom<SignedHeaderHashed> for WireUpdateEntry {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        let d: UpdateEntry = h.into_content().try_into()?;
        Ok(Self {
            signature,
            timestamp: d.timestamp,
            author: d.author,
            header_seq: d.header_seq,
            prev_header: d.prev_header,
            original_entry_address: d.original_entry_address,
            original_header_address: d.original_header_address,
        })
    }
}

impl TryFrom<SignedHeaderHashed> for WireUpdateEntryRelationship {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        let d: UpdateEntry = h.into_content().try_into()?;
        Ok(Self {
            signature,
            timestamp: d.timestamp,
            author: d.author,
            header_seq: d.header_seq,
            prev_header: d.prev_header,
            original_header_address: d.original_header_address,
            new_entry_address: d.entry_hash,
            new_entry_type: d.entry_type,
        })
    }
}

impl WireNewEntryHeader {
    pub async fn into_element(self, entry_type: EntryType, entry: Entry) -> Element {
        let entry_hash = EntryHash::with_data_sync(&entry);
        Element::new(self.into_header(entry_type, entry_hash).await, Some(entry))
    }

    pub async fn into_header(
        self,
        entry_type: EntryType,
        entry_hash: EntryHash,
    ) -> SignedHeaderHashed {
        match self {
            WireNewEntryHeader::Create(ec) => {
                let signature = ec.signature;
                let ec = CreateEntry {
                    author: ec.author,
                    timestamp: ec.timestamp,
                    header_seq: ec.header_seq,
                    prev_header: ec.prev_header,
                    entry_type,
                    entry_hash,
                };
                SignedHeaderHashed::from_content_sync(SignedHeader(ec.into(), signature))
            }
            WireNewEntryHeader::Update(eu) => {
                let signature = eu.signature;
                let eu = UpdateEntry {
                    author: eu.author,
                    timestamp: eu.timestamp,
                    header_seq: eu.header_seq,
                    prev_header: eu.prev_header,
                    original_entry_address: eu.original_entry_address,
                    original_header_address: eu.original_header_address,
                    entry_type,
                    entry_hash,
                };
                SignedHeaderHashed::from_content_sync(SignedHeader(eu.into(), signature))
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
            Header::CreateEntry(ec) => Ok(Self::Create((ec, s).into())),
            Header::UpdateEntry(eu) => Ok(Self::Update((eu, s).into())),
            _ => Err(HeaderError::NotNewEntry),
        }
    }
}

impl TryFrom<Header> for NewEntryHeader {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::CreateEntry(h) => Ok(NewEntryHeader::Create(h)),
            Header::UpdateEntry(h) => Ok(NewEntryHeader::Update(h)),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for NewEntryHeaderRef<'a> {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::CreateEntry(h) => Ok(NewEntryHeaderRef::Create(h)),
            Header::UpdateEntry(h) => Ok(NewEntryHeaderRef::Update(h)),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> From<&'a NewEntryHeader> for NewEntryHeaderRef<'a> {
    fn from(n: &'a NewEntryHeader) -> Self {
        match n {
            NewEntryHeader::Create(ec) => NewEntryHeaderRef::Create(ec),
            NewEntryHeader::Update(eu) => NewEntryHeaderRef::Update(eu),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fixt::HeaderBuilderCommonFixturator,
        test_utils::{fake_dna_hash, fake_entry_hash},
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
        let bytes = holochain_serialized_bytes::encode(&orig).unwrap();
        let res: Header = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_msgpack_roundtrip() {
        let orig: Header = CreateEntry::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_hash(1).into(),
        )
        .into();
        let bytes = holochain_serialized_bytes::encode(&orig).unwrap();
        println!("{:?}", bytes);
        let res: Header = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_serializedbytes_roundtrip() {
        let orig: Header = CreateEntry::from_builder(
            HeaderBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_hash(1).into(),
        )
        .into();
        let bytes: SerializedBytes = orig.clone().try_into().unwrap();
        let res: Header = bytes.try_into().unwrap();
        assert_eq!(orig, res);
    }
}
