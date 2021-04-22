//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use std::str::FromStr;

use crate::element::ElementGroup;
use crate::header::NewEntryHeader;
use crate::prelude::*;
use error::DhtOpError;
use error::DhtOpResult;
use holo_hash::hash_type;
use holo_hash::HashableContentBytes;
use holochain_sqlite::rusqlite::types::FromSql;
use holochain_sqlite::rusqlite::ToSql;
use holochain_zome_types::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[allow(missing_docs)]
pub mod error;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, derive_more::Display,
)]
pub enum DhtOp {
    #[display(fmt = "StoreElement")]
    /// Used to notify the authority for a header that it has been created.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that the element passes validation.
    /// - Store the header into their DHT shard.
    /// - Store the entry into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that entry up-to-date.
    StoreElement(Signature, Header, Option<Box<Entry>>),

    #[display(fmt = "StoreEntry")]
    /// Used to notify the authority for an entry that it has been created
    /// anew. (The same entry can be created more than once.)
    ///
    /// Conceptually, authorities receiving this `DhtOp` do four things:
    ///
    /// - Ensure that the element passes validation.
    /// - Store the entry into their DHT shard.
    /// - Store the header into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that header up-to-date.
    /// - Add a "created-by" reference from the entry to the hash of the header.
    ///
    /// TODO: document how those "created-by" references are stored in
    /// reality.
    StoreEntry(Signature, NewEntryHeader, Box<Entry>),

    #[display(fmt = "RegisterAgentActivity")]
    /// Used to notify the authority for an agent's public key that that agent
    /// has committed a new header.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that *the header alone* passes surface-level validation.
    /// - Store the header into their DHT shard.
    ///   - FIXME: @artbrock, do they?
    /// - Add an "agent-activity" reference from the public key to the hash
    ///   of the header.
    ///
    /// TODO: document how those "agent-activity" references are stored in
    /// reality.
    RegisterAgentActivity(Signature, Header),

    #[display(fmt = "RegisterUpdatedContent")]
    /// Op for updating an entry.
    /// This is sent to the entry authority.
    // TODO: This entry is here for validation by the entry update header holder
    // link's don't do this. The entry is validated by store entry. Maybe we either
    // need to remove the Entry here or add it to link.
    RegisterUpdatedContent(Signature, header::Update, Option<Box<Entry>>),

    #[display(fmt = "RegisterUpdatedElement")]
    /// Op for updating an element.
    /// This is sent to the element authority.
    RegisterUpdatedElement(Signature, header::Update, Option<Box<Entry>>),

    #[display(fmt = "RegisterDeletedBy")]
    /// Op for registering a Header deletion with the Header authority
    RegisterDeletedBy(Signature, header::Delete),

    #[display(fmt = "RegisterDeletedEntryHeader")]
    /// Op for registering a Header deletion with the Entry authority, so that
    /// the Entry can be marked Dead if all of its Headers have been deleted
    RegisterDeletedEntryHeader(Signature, header::Delete),

    #[display(fmt = "RegisterAddLink")]
    /// Op for adding a link
    RegisterAddLink(Signature, header::CreateLink),

    #[display(fmt = "RegisterRemoveLink")]
    /// Op for removing a link
    RegisterRemoveLink(Signature, header::DeleteLink),
}

/// Show that this type is used as the basis
type DhtBasis = AnyDhtHash;

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, derive_more::Display)]
pub enum DhtOpLight {
    #[display(fmt = "StoreElement")]
    StoreElement(HeaderHash, Option<EntryHash>, DhtBasis),
    #[display(fmt = "StoreEntry")]
    StoreEntry(HeaderHash, EntryHash, DhtBasis),
    #[display(fmt = "RegisterAgentActivity")]
    RegisterAgentActivity(HeaderHash, DhtBasis),
    #[display(fmt = "RegisterUpdatedContent")]
    RegisterUpdatedContent(HeaderHash, EntryHash, DhtBasis),
    #[display(fmt = "RegisterUpdatedElement")]
    RegisterUpdatedElement(HeaderHash, EntryHash, DhtBasis),
    #[display(fmt = "RegisterDeletedBy")]
    RegisterDeletedBy(HeaderHash, DhtBasis),
    #[display(fmt = "RegisterDeletedEntryHeader")]
    RegisterDeletedEntryHeader(HeaderHash, DhtBasis),
    #[display(fmt = "RegisterAddLink")]
    RegisterAddLink(HeaderHash, DhtBasis),
    #[display(fmt = "RegisterRemoveLink")]
    RegisterRemoveLink(HeaderHash, DhtBasis),
}

impl PartialEq for DhtOpLight {
    fn eq(&self, other: &Self) -> bool {
        // The ops are the same if they are the same type on the same header hash.
        // We can't derive eq because `Option<EntryHash>` doesn't make the op different.
        // We can ignore the basis because the basis is derived from the header and op type.
        self.get_type() == other.get_type() && self.header_hash() == other.header_hash()
    }
}

impl Eq for DhtOpLight {}

impl std::hash::Hash for DhtOpLight {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_type().hash(state);
        self.header_hash().hash(state);
    }
}

/// This enum is used to
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    strum_macros::EnumString,
)]
pub enum DhtOpType {
    #[display(fmt = "StoreElement")]
    StoreElement,
    #[display(fmt = "StoreEntry")]
    StoreEntry,
    #[display(fmt = "RegisterAgentActivity")]
    RegisterAgentActivity,
    #[display(fmt = "RegisterUpdatedContent")]
    RegisterUpdatedContent,
    #[display(fmt = "RegisterUpdatedElement")]
    RegisterUpdatedElement,
    #[display(fmt = "RegisterDeletedBy")]
    RegisterDeletedBy,
    #[display(fmt = "RegisterDeletedEntryHeader")]
    RegisterDeletedEntryHeader,
    #[display(fmt = "RegisterAddLink")]
    RegisterAddLink,
    #[display(fmt = "RegisterRemoveLink")]
    RegisterRemoveLink,
}

impl ToSql for DhtOpType {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

impl FromSql for DhtOpType {
    fn column_result(
        value: holochain_sqlite::rusqlite::types::ValueRef<'_>,
    ) -> holochain_sqlite::rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).and_then(|string| {
            DhtOpType::from_str(&string)
                .map_err(|_| holochain_sqlite::rusqlite::types::FromSqlError::InvalidType)
        })
    }
}

impl DhtOp {
    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::StoreElement(_, header, _) => UniqueForm::StoreElement(header),
            Self::StoreEntry(_, header, _) => UniqueForm::StoreEntry(header),
            Self::RegisterAgentActivity(_, header) => UniqueForm::RegisterAgentActivity(header),
            Self::RegisterUpdatedContent(_, header, _) => {
                UniqueForm::RegisterUpdatedContent(header)
            }
            Self::RegisterUpdatedElement(_, header, _) => {
                UniqueForm::RegisterUpdatedElement(header)
            }
            Self::RegisterDeletedBy(_, header) => UniqueForm::RegisterDeletedBy(header),
            Self::RegisterDeletedEntryHeader(_, header) => {
                UniqueForm::RegisterDeletedEntryHeader(header)
            }
            Self::RegisterAddLink(_, header) => UniqueForm::RegisterAddLink(header),
            Self::RegisterRemoveLink(_, header) => UniqueForm::RegisterRemoveLink(header),
        }
    }

    /// Returns the basis hash which determines which agents will receive this DhtOp
    pub fn dht_basis(&self) -> AnyDhtHash {
        self.as_unique_form().basis()
    }

    /// Convert a [DhtOp] to a [DhtOpLight] and basis
    pub fn to_light(
        // Hoping one day we can work out how to go from `&Create`
        // to `&Header::Create(Create)` so punting on a reference
        &self,
    ) -> DhtOpLight {
        let basis = self.dht_basis();
        match self {
            DhtOp::StoreElement(_, h, _) => {
                let e = h.entry_data().map(|(e, _)| e.clone());
                let h = HeaderHash::with_data_sync(h);
                DhtOpLight::StoreElement(h, e, basis)
            }
            DhtOp::StoreEntry(_, h, _) => {
                let e = h.entry().clone();
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::StoreEntry(h, e, basis)
            }
            DhtOp::RegisterAgentActivity(_, h) => {
                let h = HeaderHash::with_data_sync(h);
                DhtOpLight::RegisterAgentActivity(h, basis)
            }
            DhtOp::RegisterUpdatedContent(_, h, _) => {
                let e = h.entry_hash.clone();
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterUpdatedContent(h, e, basis)
            }
            DhtOp::RegisterUpdatedElement(_, h, _) => {
                let e = h.entry_hash.clone();
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterUpdatedElement(h, e, basis)
            }
            DhtOp::RegisterDeletedBy(_, h) => {
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterDeletedBy(h, basis)
            }
            DhtOp::RegisterDeletedEntryHeader(_, h) => {
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterDeletedEntryHeader(h, basis)
            }
            DhtOp::RegisterAddLink(_, h) => {
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterAddLink(h, basis)
            }
            DhtOp::RegisterRemoveLink(_, h) => {
                let h = HeaderHash::with_data_sync(&Header::from(h.clone()));
                DhtOpLight::RegisterRemoveLink(h, basis)
            }
        }
    }

    /// Get the signature for this op
    pub fn signature(&self) -> &Signature {
        match self {
            DhtOp::StoreElement(s, _, _)
            | DhtOp::StoreEntry(s, _, _)
            | DhtOp::RegisterAgentActivity(s, _)
            | DhtOp::RegisterUpdatedContent(s, _, _)
            | DhtOp::RegisterUpdatedElement(s, _, _)
            | DhtOp::RegisterDeletedBy(s, _)
            | DhtOp::RegisterDeletedEntryHeader(s, _)
            | DhtOp::RegisterAddLink(s, _)
            | DhtOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Extract inner Signature, Header and Option<Entry> from an op
    pub fn into_inner(self) -> (Signature, Header, Option<Entry>) {
        match self {
            DhtOp::StoreElement(s, h, e) => (s, h, e.map(|e| *e)),
            DhtOp::StoreEntry(s, h, e) => (s, h.into(), Some(*e)),
            DhtOp::RegisterAgentActivity(s, h) => (s, h, None),
            DhtOp::RegisterUpdatedContent(s, h, e) => (s, h.into(), e.map(|e| *e)),
            DhtOp::RegisterUpdatedElement(s, h, e) => (s, h.into(), e.map(|e| *e)),
            DhtOp::RegisterDeletedBy(s, h) => (s, h.into(), None),
            DhtOp::RegisterDeletedEntryHeader(s, h) => (s, h.into(), None),
            DhtOp::RegisterAddLink(s, h) => (s, h.into(), None),
            DhtOp::RegisterRemoveLink(s, h) => (s, h.into(), None),
        }
    }

    /// Get the header from this op
    /// This requires cloning and converting the header
    /// as some ops don't hold the Header type
    pub fn header(&self) -> Header {
        match self {
            DhtOp::StoreElement(_, h, _) => h.clone(),
            DhtOp::StoreEntry(_, h, _) => h.clone().into(),
            DhtOp::RegisterAgentActivity(_, h) => h.clone(),
            DhtOp::RegisterUpdatedContent(_, h, _) => h.clone().into(),
            DhtOp::RegisterUpdatedElement(_, h, _) => h.clone().into(),
            DhtOp::RegisterDeletedBy(_, h) => h.clone().into(),
            DhtOp::RegisterDeletedEntryHeader(_, h) => h.clone().into(),
            DhtOp::RegisterAddLink(_, h) => h.clone().into(),
            DhtOp::RegisterRemoveLink(_, h) => h.clone().into(),
        }
    }

    /// Get the entry from this op, if one exists
    pub fn entry(&self) -> Option<&Entry> {
        match self {
            DhtOp::StoreElement(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::StoreEntry(_, _, e) => Some(&*e),
            DhtOp::RegisterUpdatedContent(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::RegisterUpdatedElement(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::RegisterAgentActivity(_, _) => None,
            DhtOp::RegisterDeletedBy(_, _) => None,
            DhtOp::RegisterDeletedEntryHeader(_, _) => None,
            DhtOp::RegisterAddLink(_, _) => None,
            DhtOp::RegisterRemoveLink(_, _) => None,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            DhtOp::StoreElement(_, _, _) => DhtOpType::StoreElement,
            DhtOp::StoreEntry(_, _, _) => DhtOpType::StoreEntry,
            DhtOp::RegisterUpdatedContent(_, _, _) => DhtOpType::RegisterUpdatedContent,
            DhtOp::RegisterUpdatedElement(_, _, _) => DhtOpType::RegisterUpdatedElement,
            DhtOp::RegisterAgentActivity(_, _) => DhtOpType::RegisterAgentActivity,
            DhtOp::RegisterDeletedBy(_, _) => DhtOpType::RegisterDeletedBy,
            DhtOp::RegisterDeletedEntryHeader(_, _) => DhtOpType::RegisterDeletedEntryHeader,
            DhtOp::RegisterAddLink(_, _) => DhtOpType::RegisterAddLink,
            DhtOp::RegisterRemoveLink(_, _) => DhtOpType::RegisterRemoveLink,
        }
    }

    /// From a type, header and an entry (if there is one)
    pub fn from_type(
        op_type: DhtOpType,
        header: SignedHeader,
        entry: Option<Entry>,
    ) -> DhtOpResult<Self> {
        let SignedHeader(header, signature) = header;
        let r = match op_type {
            DhtOpType::StoreElement => DhtOp::StoreElement(signature, header, entry.map(Box::new)),
            DhtOpType::StoreEntry => {
                let entry = entry.ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?;
                let header = match header {
                    Header::Create(c) => NewEntryHeader::Create(c),
                    Header::Update(c) => NewEntryHeader::Update(c),
                    _ => return Err(DhtOpError::OpHeaderMismatch(op_type, header.header_type())),
                };
                DhtOp::StoreEntry(signature, header, Box::new(entry))
            }
            DhtOpType::RegisterAgentActivity => DhtOp::RegisterAgentActivity(signature, header),
            DhtOpType::RegisterUpdatedContent => {
                DhtOp::RegisterUpdatedContent(signature, header.try_into()?, entry.map(Box::new))
            }
            DhtOpType::RegisterUpdatedElement => {
                DhtOp::RegisterUpdatedElement(signature, header.try_into()?, entry.map(Box::new))
            }
            DhtOpType::RegisterDeletedBy => DhtOp::RegisterDeletedBy(signature, header.try_into()?),
            DhtOpType::RegisterDeletedEntryHeader => {
                DhtOp::RegisterDeletedBy(signature, header.try_into()?)
            }
            DhtOpType::RegisterAddLink => DhtOp::RegisterAddLink(signature, header.try_into()?),
            DhtOpType::RegisterRemoveLink => {
                DhtOp::RegisterRemoveLink(signature, header.try_into()?)
            }
        };
        Ok(r)
    }
}

impl DhtOpLight {
    /// Get the dht basis for where to send this op
    pub fn dht_basis(&self) -> &AnyDhtHash {
        match self {
            DhtOpLight::StoreElement(_, _, b)
            | DhtOpLight::StoreEntry(_, _, b)
            | DhtOpLight::RegisterAgentActivity(_, b)
            | DhtOpLight::RegisterUpdatedContent(_, _, b)
            | DhtOpLight::RegisterUpdatedElement(_, _, b)
            | DhtOpLight::RegisterDeletedBy(_, b)
            | DhtOpLight::RegisterDeletedEntryHeader(_, b)
            | DhtOpLight::RegisterAddLink(_, b)
            | DhtOpLight::RegisterRemoveLink(_, b) => b,
        }
    }
    /// Get the header hash from this op
    pub fn header_hash(&self) -> &HeaderHash {
        match self {
            DhtOpLight::StoreElement(h, _, _)
            | DhtOpLight::StoreEntry(h, _, _)
            | DhtOpLight::RegisterAgentActivity(h, _)
            | DhtOpLight::RegisterUpdatedContent(h, _, _)
            | DhtOpLight::RegisterUpdatedElement(h, _, _)
            | DhtOpLight::RegisterDeletedBy(h, _)
            | DhtOpLight::RegisterDeletedEntryHeader(h, _)
            | DhtOpLight::RegisterAddLink(h, _)
            | DhtOpLight::RegisterRemoveLink(h, _) => h,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            DhtOpLight::StoreElement(_, _, _) => DhtOpType::StoreElement,
            DhtOpLight::StoreEntry(_, _, _) => DhtOpType::StoreEntry,
            DhtOpLight::RegisterUpdatedContent(_, _, _) => DhtOpType::RegisterUpdatedContent,
            DhtOpLight::RegisterUpdatedElement(_, _, _) => DhtOpType::RegisterUpdatedElement,
            DhtOpLight::RegisterAgentActivity(_, _) => DhtOpType::RegisterAgentActivity,
            DhtOpLight::RegisterDeletedBy(_, _) => DhtOpType::RegisterDeletedBy,
            DhtOpLight::RegisterDeletedEntryHeader(_, _) => DhtOpType::RegisterDeletedEntryHeader,
            DhtOpLight::RegisterAddLink(_, _) => DhtOpType::RegisterAddLink,
            DhtOpLight::RegisterRemoveLink(_, _) => DhtOpType::RegisterRemoveLink,
        }
    }

    /// From a type with the hashes.
    pub fn from_type(
        op_type: DhtOpType,
        header_hash: HeaderHash,
        header: &Header,
    ) -> DhtOpResult<Self> {
        let op = match op_type {
            DhtOpType::StoreElement => {
                let entry_hash = header.entry_hash().cloned();
                Self::StoreElement(header_hash.clone(), entry_hash, header_hash.into())
            }
            DhtOpType::StoreEntry => {
                let entry_hash = header
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?;
                Self::StoreEntry(header_hash, entry_hash.clone(), entry_hash.into())
            }
            DhtOpType::RegisterAgentActivity => {
                Self::RegisterAgentActivity(header_hash, header.author().clone().into())
            }
            DhtOpType::RegisterUpdatedContent => {
                let entry_hash = header
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?;
                let basis = match header {
                    Header::Update(update) => update.original_entry_address.clone(),
                    _ => return Err(DhtOpError::OpHeaderMismatch(op_type, header.header_type())),
                };
                Self::RegisterUpdatedContent(header_hash, entry_hash, basis.into())
            }
            DhtOpType::RegisterUpdatedElement => {
                let entry_hash = header
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?;
                let basis = match header {
                    Header::Update(update) => update.original_entry_address.clone(),
                    _ => return Err(DhtOpError::OpHeaderMismatch(op_type, header.header_type())),
                };
                Self::RegisterUpdatedElement(header_hash, entry_hash, basis.into())
            }
            DhtOpType::RegisterDeletedBy => {
                Self::RegisterDeletedBy(header_hash.clone(), header_hash.into())
            }
            DhtOpType::RegisterDeletedEntryHeader => {
                let basis = header
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?
                    .clone();
                Self::RegisterDeletedBy(header_hash, basis.into())
            }
            DhtOpType::RegisterAddLink => {
                let basis = match header {
                    Header::CreateLink(create_link) => create_link.base_address.clone(),
                    _ => return Err(DhtOpError::OpHeaderMismatch(op_type, header.header_type())),
                };
                Self::RegisterAddLink(header_hash, basis.into())
            }
            DhtOpType::RegisterRemoveLink => {
                let basis = match header {
                    Header::DeleteLink(delete_link) => delete_link.base_address.clone(),
                    _ => return Err(DhtOpError::OpHeaderMismatch(op_type, header.header_type())),
                };
                Self::RegisterRemoveLink(header_hash, basis.into())
            }
        };
        Ok(op)
    }
}

// FIXME: need to use this in HashableContent
#[allow(missing_docs)]
#[derive(Serialize, Debug)]
pub enum UniqueForm<'a> {
    // As an optimization, we don't include signatures. They would be redundant
    // with headers and therefore would waste hash/comparison time to include.
    StoreElement(&'a Header),
    StoreEntry(&'a NewEntryHeader),
    RegisterAgentActivity(&'a Header),
    RegisterUpdatedContent(&'a header::Update),
    RegisterUpdatedElement(&'a header::Update),
    RegisterDeletedBy(&'a header::Delete),
    RegisterDeletedEntryHeader(&'a header::Delete),
    RegisterAddLink(&'a header::CreateLink),
    RegisterRemoveLink(&'a header::DeleteLink),
}

impl<'a> UniqueForm<'a> {
    fn basis(&'a self) -> AnyDhtHash {
        match self {
            UniqueForm::StoreElement(header) => HeaderHash::with_data_sync(*header).into(),
            UniqueForm::StoreEntry(header) => header.entry().clone().into(),
            UniqueForm::RegisterAgentActivity(header) => header.author().clone().into(),
            UniqueForm::RegisterUpdatedContent(header) => {
                header.original_entry_address.clone().into()
            }
            UniqueForm::RegisterUpdatedElement(header) => {
                header.original_header_address.clone().into()
            }
            UniqueForm::RegisterDeletedBy(header) => header.deletes_address.clone().into(),
            UniqueForm::RegisterDeletedEntryHeader(header) => {
                header.deletes_entry_address.clone().into()
            }
            UniqueForm::RegisterAddLink(header) => header.base_address.clone().into(),
            UniqueForm::RegisterRemoveLink(header) => header.base_address.clone().into(),
        }
    }

    /// Get the dht op hash without cloning the header.
    pub fn op_hash(op_type: DhtOpType, header: Header) -> DhtOpResult<(Header, DhtOpHash)> {
        match op_type {
            DhtOpType::StoreElement => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreElement(&header));
                Ok((header, hash))
            }
            DhtOpType::StoreEntry => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreEntry(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterAgentActivity => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAgentActivity(&header));
                Ok((header, hash))
            }
            DhtOpType::RegisterUpdatedContent => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedContent(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterUpdatedElement => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedElement(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterDeletedBy => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedBy(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterDeletedEntryHeader => {
                let header = header.try_into()?;
                let hash =
                    DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedEntryHeader(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterAddLink => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAddLink(&header));
                Ok((header.into(), hash))
            }
            DhtOpType::RegisterRemoveLink => {
                let header = header.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterRemoveLink(&header));
                Ok((header.into(), hash))
            }
        }
    }
}

/// Produce all DhtOps for a Element
pub fn produce_ops_from_element(element: &Element) -> DhtOpResult<Vec<DhtOp>> {
    let op_lights = produce_op_lights_from_elements(vec![element])?;
    let (shh, maybe_entry) = element.clone().into_inner();
    let (header, signature): (Header, Signature) = shh.into_inner().0.into();

    let mut ops = Vec::with_capacity(op_lights.len());

    for op_light in op_lights {
        let signature = signature.clone();
        let header = header.clone();
        let op = match op_light {
            DhtOpLight::StoreElement(_, _, _) => {
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::StoreElement(signature, header, maybe_entry_box)
            }
            DhtOpLight::StoreEntry(_, _, _) => {
                let new_entry_header = header.clone().try_into()?;
                let box_entry = match maybe_entry.clone().into_option() {
                    Some(entry) => Box::new(entry),
                    None => {
                        // Entry is private so continue
                        continue;
                    }
                };
                DhtOp::StoreEntry(signature, new_entry_header, box_entry)
            }
            DhtOpLight::RegisterAgentActivity(_, _) => {
                DhtOp::RegisterAgentActivity(signature, header)
            }
            DhtOpLight::RegisterUpdatedContent(_, _, _) => {
                let entry_update = header.try_into()?;
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::RegisterUpdatedContent(signature, entry_update, maybe_entry_box)
            }
            DhtOpLight::RegisterUpdatedElement(_, _, _) => {
                let entry_update = header.try_into()?;
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::RegisterUpdatedElement(signature, entry_update, maybe_entry_box)
            }
            DhtOpLight::RegisterDeletedEntryHeader(_, _) => {
                let element_delete = header.try_into()?;
                DhtOp::RegisterDeletedEntryHeader(signature, element_delete)
            }
            DhtOpLight::RegisterDeletedBy(_, _) => {
                let element_delete = header.try_into()?;
                DhtOp::RegisterDeletedBy(signature, element_delete)
            }
            DhtOpLight::RegisterAddLink(_, _) => {
                let link_add = header.try_into()?;
                DhtOp::RegisterAddLink(signature, link_add)
            }
            DhtOpLight::RegisterRemoveLink(_, _) => {
                let link_remove = header.try_into()?;
                DhtOp::RegisterRemoveLink(signature, link_remove)
            }
        };
        ops.push(op);
    }
    Ok(ops)
}

/// Produce all the op lights for tese elements
pub fn produce_op_lights_from_elements(headers: Vec<&Element>) -> DhtOpResult<Vec<DhtOpLight>> {
    let length = headers.len();
    let headers_and_hashes = headers.into_iter().map(|e| {
        (
            e.header_address(),
            e.header(),
            e.header().entry_data().map(|(h, _)| h.clone()),
        )
    });
    produce_op_lights_from_iter(headers_and_hashes, length)
}

/// Produce all the op lights from this element group
/// with a shared entry
pub fn produce_op_lights_from_element_group(
    elements: &ElementGroup<'_>,
) -> DhtOpResult<Vec<DhtOpLight>> {
    let len = elements.len();
    let headers_and_hashes = elements.headers_and_hashes();
    let maybe_entry_hash = Some(elements.entry_hash());
    produce_op_lights_from_parts(headers_and_hashes, maybe_entry_hash, len)
}

/// Data minimal clone (no cloning entries) cheap &Element to DhtOpLight conversion
fn produce_op_lights_from_parts<'a>(
    headers_and_hashes: impl Iterator<Item = (&'a HeaderHash, &'a Header)>,
    maybe_entry_hash: Option<&EntryHash>,
    length: usize,
) -> DhtOpResult<Vec<DhtOpLight>> {
    let iter = headers_and_hashes.map(|(head, hash)| (head, hash, maybe_entry_hash.cloned()));
    produce_op_lights_from_iter(iter, length)
}
fn produce_op_lights_from_iter<'a>(
    iter: impl Iterator<Item = (&'a HeaderHash, &'a Header, Option<EntryHash>)>,
    length: usize,
) -> DhtOpResult<Vec<DhtOpLight>> {
    // Each header will have at least 2 ops
    let mut ops = Vec::with_capacity(length * 2);

    for (header_hash, header, maybe_entry_hash) in iter {
        let header_hash = header_hash.clone();

        let store_element_basis = UniqueForm::StoreElement(header).basis();
        let register_activity_basis = UniqueForm::RegisterAgentActivity(header).basis();

        ops.push(DhtOpLight::StoreElement(
            header_hash.clone(),
            maybe_entry_hash.clone(),
            store_element_basis,
        ));
        ops.push(DhtOpLight::RegisterAgentActivity(
            header_hash.clone(),
            register_activity_basis,
        ));

        match header {
            Header::Dna(_)
            | Header::OpenChain(_)
            | Header::CloseChain(_)
            | Header::AgentValidationPkg(_)
            | Header::InitZomesComplete(_) => {}
            Header::CreateLink(link_add) => ops.push(DhtOpLight::RegisterAddLink(
                header_hash,
                UniqueForm::RegisterAddLink(link_add).basis(),
            )),
            Header::DeleteLink(link_remove) => ops.push(DhtOpLight::RegisterRemoveLink(
                header_hash,
                UniqueForm::RegisterRemoveLink(link_remove).basis(),
            )),
            Header::Create(entry_create) => ops.push(DhtOpLight::StoreEntry(
                header_hash,
                maybe_entry_hash.ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?,
                UniqueForm::StoreEntry(&NewEntryHeader::Create(entry_create.clone())).basis(),
            )),
            Header::Update(entry_update) => {
                let entry_hash = maybe_entry_hash
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone()))?;
                ops.push(DhtOpLight::StoreEntry(
                    header_hash.clone(),
                    entry_hash.clone(),
                    UniqueForm::StoreEntry(&NewEntryHeader::Update(entry_update.clone())).basis(),
                ));
                ops.push(DhtOpLight::RegisterUpdatedContent(
                    header_hash.clone(),
                    entry_hash.clone(),
                    UniqueForm::RegisterUpdatedContent(entry_update).basis(),
                ));
                ops.push(DhtOpLight::RegisterUpdatedElement(
                    header_hash,
                    entry_hash,
                    UniqueForm::RegisterUpdatedElement(entry_update).basis(),
                ));
            }
            Header::Delete(entry_delete) => {
                // TODO: VALIDATION: This only works if entry_delete.remove_address is either Create
                // or Update
                ops.push(DhtOpLight::RegisterDeletedBy(
                    header_hash.clone(),
                    UniqueForm::RegisterDeletedBy(entry_delete).basis(),
                ));
                ops.push(DhtOpLight::RegisterDeletedEntryHeader(
                    header_hash,
                    UniqueForm::RegisterDeletedEntryHeader(entry_delete).basis(),
                ));
            }
        }
    }
    Ok(ops)
}

// This has to be done manually because the macro
// implements both directions and that isn't possible with references
// TODO: Maybe add a one-way version to holochain_serialized_bytes?
impl<'a> TryFrom<&UniqueForm<'a>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(u: &UniqueForm<'a>) -> Result<Self, Self::Error> {
        match holochain_serialized_bytes::encode(u) {
            Ok(v) => Ok(SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(v),
            )),
            Err(e) => Err(SerializedBytesError::Serialize(e.to_string())),
        }
    }
}

/// A DhtOp paired with its DhtOpHash
pub type DhtOpHashed = HoloHashed<DhtOp>;

impl HashableContent for DhtOp {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            (&self.as_unique_form())
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl HashableContent for UniqueForm<'_> {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            self.try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
/// Condensed version of ops for sending across the wire.
pub enum WireOps {
    /// Response for get entry.
    Entry(WireEntryOps),
    /// Response for get element.
    Element(WireElementOps),
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// The data rendered from a wire op to place in the database.
pub struct RenderedOp {
    /// The header to insert into the database.
    pub header: SignedHeaderHashed,
    /// The header to insert into the database.
    pub op_light: DhtOpLight,
    /// The hash of the [`DhtOp`]
    pub op_hash: DhtOpHash,
    /// The validation status of the header.
    pub validation_status: Option<ValidationStatus>,
}

impl RenderedOp {
    /// Try to create a new rendered op from wire data.
    /// This function computes all the hashes and
    /// reconstructs the full headers.
    pub fn new(
        header: Header,
        signature: Signature,
        validation_status: Option<ValidationStatus>,
        op_type: DhtOpType,
    ) -> DhtOpResult<Self> {
        let (header, op_hash) = UniqueForm::op_hash(op_type, header)?;
        let header_hashed = HeaderHashed::from_content_sync(header);
        // TODO: Verify signature?
        let header = SignedHeaderHashed::with_presigned(header_hashed, signature);
        let op_light = DhtOpLight::from_type(op_type, header.as_hash().clone(), header.header())?;
        Ok(Self {
            header,
            op_light,
            validation_status,
            op_hash,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
/// The full data for insertion into the database.
/// The reason we don't use [`DhtOp`] is because we don't
/// want to clone the entry for every header.
pub struct RenderedOps {
    /// Entry for the ops if there is one.
    pub entry: Option<EntryHashed>,
    /// Op data to insert.
    pub ops: Vec<RenderedOp>,
}

/// Type for deriving ordering of DhtOps
/// Don't change the order of this enum unless
/// you mean to change the order we process ops
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum OpNumericalOrder {
    RegisterAgentActivity = 0,
    StoreEntry,
    StoreElement,
    RegisterUpdatedContent,
    RegisterUpdatedElement,
    RegisterDeletedBy,
    RegisterDeletedEntryHeader,
    RegisterAddLink,
    RegisterRemoveLink,
}

/// This is used as an index for ordering ops in our database.
/// It gives the most likely ordering where dependencies will come
/// first.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OpOrder {
    order: OpNumericalOrder,
    timestamp: holochain_zome_types::timestamp::Timestamp,
}

impl std::fmt::Display for OpOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.order as u8, self.timestamp.0, self.timestamp.1
        )
    }
}

impl OpOrder {
    /// Create a new ordering from a op type and timestamp.
    pub fn new(op_type: DhtOpType, timestamp: holochain_zome_types::timestamp::Timestamp) -> Self {
        let order = match op_type {
            DhtOpType::StoreElement => OpNumericalOrder::StoreElement,
            DhtOpType::StoreEntry => OpNumericalOrder::StoreEntry,
            DhtOpType::RegisterAgentActivity => OpNumericalOrder::RegisterAgentActivity,
            DhtOpType::RegisterUpdatedContent => OpNumericalOrder::RegisterUpdatedContent,
            DhtOpType::RegisterUpdatedElement => OpNumericalOrder::RegisterUpdatedElement,
            DhtOpType::RegisterDeletedBy => OpNumericalOrder::RegisterDeletedBy,
            DhtOpType::RegisterDeletedEntryHeader => OpNumericalOrder::RegisterDeletedEntryHeader,
            DhtOpType::RegisterAddLink => OpNumericalOrder::RegisterAddLink,
            DhtOpType::RegisterRemoveLink => OpNumericalOrder::RegisterRemoveLink,
        };
        Self { order, timestamp }
    }
}

impl holochain_sqlite::rusqlite::ToSql for OpOrder {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            self.to_string().into(),
        ))
    }
}
