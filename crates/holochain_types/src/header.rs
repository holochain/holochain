//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::element::ElementStatus;
use crate::element::SignedHeaderHashedExt;
use crate::prelude::*;
use conversions::WrongHeaderError;
use derive_more::From;
use holo_hash::EntryHash;
use holochain_zome_types::prelude::*;

use error::*;

pub mod error;

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash, derive_more::From,
)]
/// A header of one of the two types that create a new entry.
pub enum NewEntryHeader {
    /// A header which simply creates a new entry
    Create(Create),
    /// A header which creates a new entry that is semantically related to a
    /// previously created entry or header
    Update(Update),
}

#[allow(missing_docs)]
#[derive(Debug, From)]
/// Same as NewEntryHeader but takes headers as reference
pub enum NewEntryHeaderRef<'a> {
    Create(&'a Create),
    Update(&'a Update),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
/// A header of one of the two types that create a new entry.
pub enum WireNewEntryHeader {
    Create(WireCreate),
    Update(WireUpdate),
}

#[derive(
    Debug, Clone, derive_more::Constructor, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd,
)]
/// A header of one of the two types that create a new entry.
pub struct WireHeaderStatus<W> {
    /// Skinny header for sending over the wire.
    pub header: W,
    /// Validation status of this header.
    pub validation_status: ValidationStatus,
}

/// The minimum unique data for Create headers
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireCreate {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
    pub signature: Signature,
}

/// The minimum unique data for Update headers
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireUpdate {
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
/// `Update` headers create both a new entry and
/// a metadata relationship on the original entry.
/// This wire data represents the metadata relationship
/// which is stored on the original entry, i.e. this represents
/// the "forward" reference from the original entry to the new entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireUpdateRelationship {
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
pub struct WireDelete {
    pub delete: Delete,
    pub signature: Signature,
}

impl NewEntryHeader {
    /// Get the entry on this header
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryHeader::Create(Create { entry_hash, .. })
            | NewEntryHeader::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the visibility of this header
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryHeader::Create(Create { entry_type, .. })
            | NewEntryHeader::Update(Update { entry_type, .. }) => entry_type.visibility(),
        }
    }

    /// Get the timestamp of this header
    pub fn timestamp(&self) -> &holochain_zome_types::timestamp::Timestamp {
        match self {
            NewEntryHeader::Create(Create { timestamp, .. })
            | NewEntryHeader::Update(Update { timestamp, .. }) => timestamp,
        }
    }
}

impl From<NewEntryHeader> for Header {
    fn from(h: NewEntryHeader) -> Self {
        match h {
            NewEntryHeader::Create(h) => Header::Create(h),
            NewEntryHeader::Update(h) => Header::Update(h),
        }
    }
}

impl From<(Create, Signature)> for WireCreate {
    fn from((ec, signature): (Create, Signature)) -> Self {
        Self {
            timestamp: ec.timestamp,
            author: ec.author,
            header_seq: ec.header_seq,
            prev_header: ec.prev_header,
            signature,
        }
    }
}

impl From<(Update, Signature)> for WireUpdate {
    fn from((eu, signature): (Update, Signature)) -> Self {
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

impl WireDelete {
    pub fn into_element(self) -> Element {
        Element::new(
            SignedHeaderHashed::from_content_sync(SignedHeader(self.delete.into(), self.signature)),
            None,
        )
    }
}

impl WireUpdateRelationship {
    /// Recreate the Update Element without an Entry.
    /// Useful for creating dht ops
    pub fn into_element(self, original_entry_address: EntryHash) -> Element {
        Element::new(
            SignedHeaderHashed::from_content_sync(self.into_signed_header(original_entry_address)),
            None,
        )
    }

    /// Render the [`SignedHeader`] from the wire type
    pub fn into_signed_header(self, original_entry_address: EntryHash) -> SignedHeader {
        let eu = Update {
            author: self.author,
            timestamp: self.timestamp,
            header_seq: self.header_seq,
            prev_header: self.prev_header,
            original_header_address: self.original_header_address,
            original_entry_address,
            entry_type: self.new_entry_type,
            entry_hash: self.new_entry_address,
        };
        SignedHeader(Header::Update(eu), self.signature)
    }
}

impl NewEntryHeaderRef<'_> {
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryHeaderRef::Create(Create { entry_type, .. })
            | NewEntryHeaderRef::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            NewEntryHeaderRef::Create(Create { entry_hash, .. })
            | NewEntryHeaderRef::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    pub fn to_new_entry_header(&self) -> NewEntryHeader {
        match self {
            NewEntryHeaderRef::Create(create) => NewEntryHeader::Create((*create).to_owned()),
            NewEntryHeaderRef::Update(update) => NewEntryHeader::Update((*update).to_owned()),
        }
    }
}

impl TryFrom<SignedHeaderHashed> for WireDelete {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        Ok(Self {
            delete: h.into_content().try_into()?,
            signature,
        })
    }
}

impl TryFrom<SignedHeader> for WireDelete {
    type Error = WrongHeaderError;
    fn try_from(sh: SignedHeader) -> Result<Self, Self::Error> {
        let SignedHeader(h, signature) = sh;
        Ok(Self {
            delete: h.try_into()?,
            signature,
        })
    }
}

impl TryFrom<SignedHeaderHashed> for WireUpdate {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_header_and_signature();
        let d: Update = h.into_content().try_into()?;
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

impl TryFrom<SignedHeaderHashed> for WireUpdateRelationship {
    type Error = WrongHeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (h, s) = shh.into_header_and_signature();
        SignedHeader(h.into_content(), s).try_into()
    }
}

impl TryFrom<SignedHeader> for WireUpdateRelationship {
    type Error = WrongHeaderError;
    fn try_from(sh: SignedHeader) -> Result<Self, Self::Error> {
        let SignedHeader(h, signature) = sh;
        let d: Update = h.try_into()?;
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
    pub fn into_element(self, entry_type: EntryType, entry: Entry) -> Element {
        let entry_hash = EntryHash::with_data_sync(&entry);
        Element::new(self.into_header(entry_type, entry_hash), Some(entry))
    }

    pub fn into_header(self, entry_type: EntryType, entry_hash: EntryHash) -> SignedHeaderHashed {
        SignedHeaderHashed::from_content_sync(self.into_signed_header(entry_type, entry_hash))
    }

    pub fn into_signed_header(self, entry_type: EntryType, entry_hash: EntryHash) -> SignedHeader {
        match self {
            WireNewEntryHeader::Create(ec) => {
                let signature = ec.signature;
                let ec = Create {
                    author: ec.author,
                    timestamp: ec.timestamp,
                    header_seq: ec.header_seq,
                    prev_header: ec.prev_header,
                    entry_type,
                    entry_hash,
                };
                SignedHeader(ec.into(), signature)
            }
            WireNewEntryHeader::Update(eu) => {
                let signature = eu.signature;
                let eu = Update {
                    author: eu.author,
                    timestamp: eu.timestamp,
                    header_seq: eu.header_seq,
                    prev_header: eu.prev_header,
                    original_entry_address: eu.original_entry_address,
                    original_header_address: eu.original_header_address,
                    entry_type,
                    entry_hash,
                };
                SignedHeader(eu.into(), signature)
            }
        }
    }
}

impl WireHeaderStatus<WireNewEntryHeader> {
    pub fn into_element_status(self, entry_type: EntryType, entry: Entry) -> ElementStatus {
        ElementStatus::new(
            self.header.into_element(entry_type, entry),
            self.validation_status,
        )
    }
}

impl WireHeaderStatus<WireUpdateRelationship> {
    pub fn into_element_status(self, entry_hash: EntryHash) -> ElementStatus {
        ElementStatus::new(self.header.into_element(entry_hash), self.validation_status)
    }
}

impl WireHeaderStatus<WireDelete> {
    pub fn into_element_status(self) -> ElementStatus {
        ElementStatus::new(self.header.into_element(), self.validation_status)
    }
}

impl<H, W, E> TryFrom<(H, ValidationStatus)> for WireHeaderStatus<W>
where
    E: Into<HeaderError>,
    H: TryInto<W, Error = E>,
{
    type Error = HeaderError;

    fn try_from(value: (H, ValidationStatus)) -> Result<Self, Self::Error> {
        Ok(Self::new(value.0.try_into().map_err(Into::into)?, value.1))
    }
}

impl TryFrom<SignedHeaderHashed> for WireNewEntryHeader {
    type Error = HeaderError;
    fn try_from(shh: SignedHeaderHashed) -> Result<Self, Self::Error> {
        let (sh, _) = shh.into_inner();
        let (header, s) = sh.into();
        match header {
            Header::Create(ec) => Ok(Self::Create((ec, s).into())),
            Header::Update(eu) => Ok(Self::Update((eu, s).into())),
            _ => Err(HeaderError::NotNewEntry),
        }
    }
}

impl TryFrom<SignedHeader> for WireNewEntryHeader {
    type Error = HeaderError;
    fn try_from(sh: SignedHeader) -> Result<Self, Self::Error> {
        let (header, s) = sh.into();
        match header {
            Header::Create(ec) => Ok(Self::Create((ec, s).into())),
            Header::Update(eu) => Ok(Self::Update((eu, s).into())),
            _ => Err(HeaderError::NotNewEntry),
        }
    }
}

impl TryFrom<Header> for NewEntryHeader {
    type Error = WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Create(h) => Ok(NewEntryHeader::Create(h)),
            Header::Update(h) => Ok(NewEntryHeader::Update(h)),
            _ => Err(WrongHeaderError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Header> for NewEntryHeaderRef<'a> {
    type Error = WrongHeaderError;
    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::Create(h) => Ok(NewEntryHeaderRef::Create(h)),
            Header::Update(h) => Ok(NewEntryHeaderRef::Update(h)),
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
    use crate::fixt::HeaderBuilderCommonFixturator;
    use crate::test_utils::fake_dna_hash;
    use crate::test_utils::fake_entry_hash;
    use ::fixt::prelude::Unpredictable;

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
        let orig: Header = Create::from_builder(
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
        let orig: Header = Create::from_builder(
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
