//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use crate::element::Element;
use crate::element::ElementGroup;
use crate::header::NewEntryHeader;
use crate::prelude::*;
use error::DhtOpError;
use error::DhtOpResult;
use holo_hash::hash_type;
use holo_hash::HashableContentBytes;
use holochain_zome_types::header;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use serde::Deserialize;
use serde::Serialize;

#[allow(missing_docs)]
pub mod error;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq)]
pub enum DhtOp {
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

    /// Op for updating an entry.
    /// This is sent to the entry authority.
    // TODO: This entry is here for validation by the entry update header holder
    // link's don't do this. The entry is validated by store entry. Maybe we either
    // need to remove the Entry here or add it to link.
    RegisterUpdatedContent(Signature, header::Update, Option<Box<Entry>>),

    /// Op for updating an element.
    /// This is sent to the element authority.
    RegisterUpdatedElement(Signature, header::Update, Option<Box<Entry>>),

    /// Op for registering a Header deletion with the Header authority
    RegisterDeletedBy(Signature, header::Delete),

    /// Op for registering a Header deletion with the Entry authority, so that
    /// the Entry can be marked Dead if all of its Headers have been deleted
    RegisterDeletedEntryHeader(Signature, header::Delete),

    /// Op for adding a link
    RegisterAddLink(Signature, header::CreateLink),

    /// Op for removing a link
    RegisterRemoveLink(Signature, header::DeleteLink),
}

/// Show that this type is used as the basis
type DhtBasis = AnyDhtHash;

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum DhtOpLight {
    StoreElement(HeaderHash, Option<EntryHash>, DhtBasis),
    StoreEntry(HeaderHash, EntryHash, DhtBasis),
    RegisterAgentActivity(HeaderHash, DhtBasis),
    RegisterUpdatedContent(HeaderHash, EntryHash, DhtBasis),
    RegisterUpdatedElement(HeaderHash, EntryHash, DhtBasis),
    RegisterDeletedBy(HeaderHash, DhtBasis),
    RegisterDeletedEntryHeader(HeaderHash, DhtBasis),
    RegisterAddLink(HeaderHash, DhtBasis),
    RegisterRemoveLink(HeaderHash, DhtBasis),
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
}

// FIXME: need to use this in HashableContent
#[allow(missing_docs)]
#[derive(Serialize)]
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
            Err(e) => Err(SerializedBytesError::ToBytes(e.to_string())),
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
