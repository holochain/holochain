//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use crate::element::Element;
use crate::{header::NewEntryHeader, prelude::*};
use error::{DhtOpError, DhtOpResult};
use holo_hash::{hash_type, HashableContentBytes};
use holochain_zome_types::{header, Entry, Header};
use serde::{Deserialize, Serialize};

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

    /// Op for updating an entry
    // TODO: This entry is here for validation by the entry update header holder
    // link's don't do this. The entry is validated by store entry. Maybe we either
    // need to remove the Entry here or add it to link.
    RegisterReplacedBy(Signature, header::EntryUpdate, Option<Box<Entry>>),

    /// Op for registering a Header deletion with the Header authority
    RegisterDeletedBy(Signature, header::ElementDelete),

    /// Op for registering a Header deletion with the Entry authority, so that
    /// the Entry can be marked Dead if all of its Headers have been deleted
    RegisterDeletedEntryHeader(Signature, header::ElementDelete),

    /// Op for adding a link
    RegisterAddLink(Signature, header::LinkAdd),

    /// Op for removing a link
    RegisterRemoveLink(Signature, header::LinkRemove),
}

impl DhtOp {
    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::StoreElement(_, header, _) => UniqueForm::StoreElement(header),
            Self::StoreEntry(_, header, _) => UniqueForm::StoreEntry(header),
            Self::RegisterAgentActivity(_, header) => UniqueForm::RegisterAgentActivity(header),
            Self::RegisterReplacedBy(_, header, _) => UniqueForm::RegisterReplacedBy(header),
            Self::RegisterDeletedBy(_, header) => UniqueForm::RegisterDeletedBy(header),
            Self::RegisterDeletedEntryHeader(_, header) => {
                UniqueForm::RegisterDeletedEntryHeader(header)
            }
            Self::RegisterAddLink(_, header) => UniqueForm::RegisterAddLink(header),
            Self::RegisterRemoveLink(_, header) => UniqueForm::RegisterRemoveLink(header),
        }
    }
}

// FIXME: need to use this in HashableContent
#[derive(Serialize)]
enum UniqueForm<'a> {
    // As an optimization, we don't include signatures. They would be redundant
    // with headers and therefore would waste hash/comparison time to include.
    StoreElement(&'a Header),
    StoreEntry(&'a NewEntryHeader),
    RegisterAgentActivity(&'a Header),
    RegisterReplacedBy(&'a header::EntryUpdate),
    RegisterDeletedBy(&'a header::ElementDelete),
    RegisterDeletedEntryHeader(&'a header::ElementDelete),
    RegisterAddLink(&'a header::LinkAdd),
    RegisterRemoveLink(&'a header::LinkRemove),
}

/// Turn a chain element into a DhtOp
pub fn ops_from_element(element: &Element) -> DhtOpResult<Vec<DhtOp>> {
    // TODO: avoid cloning everything

    let (signed_header, maybe_entry) = element.clone().into_inner();
    let (header, sig) = signed_header.into_header_and_signature();
    let header: Header = header.into_content();

    // TODO: avoid allocation, we have a static maximum of four items and
    // callers simply want to iterate over the ops.
    //
    // Maybe use `ArrayVec`?
    let mut ops = vec![
        DhtOp::StoreElement(
            sig.clone(),
            header.clone(),
            maybe_entry.clone().map(Box::new),
        ),
        DhtOp::RegisterAgentActivity(sig.clone(), header.clone()),
    ];

    match &header {
        Header::Dna(_)
        | Header::ChainOpen(_)
        | Header::ChainClose(_)
        | Header::AgentValidationPkg(_)
        | Header::InitZomesComplete(_) => {}
        Header::LinkAdd(link_add) => ops.push(DhtOp::RegisterAddLink(sig, link_add.clone())),
        Header::LinkRemove(link_remove) => {
            ops.push(DhtOp::RegisterRemoveLink(sig, link_remove.clone()))
        }
        Header::EntryCreate(header) => ops.push(DhtOp::StoreEntry(
            sig,
            NewEntryHeader::Create(header.clone()),
            Box::new(
                maybe_entry.ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone().into()))?,
            ),
        )),
        Header::EntryUpdate(entry_update) => {
            let entry = maybe_entry
                .ok_or_else(|| DhtOpError::HeaderWithoutEntry(entry_update.clone().into()))?;
            ops.push(DhtOp::StoreEntry(
                sig.clone(),
                NewEntryHeader::Update(entry_update.clone()),
                Box::new(entry.clone()),
            ));
            ops.push(DhtOp::RegisterReplacedBy(
                sig,
                entry_update.clone(),
                Some(Box::new(entry)),
            ));
        }
        Header::ElementDelete(entry_delete) => {
            // TODO: VALIDATION: This only works if entry_delete.remove_address is either EntryCreate
            // or EntryUpdate
            ops.push(DhtOp::RegisterDeletedBy(sig.clone(), entry_delete.clone()));
            ops.push(DhtOp::RegisterDeletedEntryHeader(sig, entry_delete.clone()));
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
        match holochain_serialized_bytes::to_vec_named(u) {
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
