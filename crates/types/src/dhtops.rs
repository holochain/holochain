//! Dhtops
//! Holochain's DHT operations.
//!
//! See the [item-level documentation for `DhtOp`][dhtop] for more details.
//!
//! [dhtop]: enum.DhtOp.html

use crate::element::{ChainElement, SignedHeader};
use crate::{composite_hash::AnyDhtHash, header, prelude::*, Header};
use error::{DhtOpError, DhtOpResult};
use header::NewEntryHeader;
use holochain_zome_types::Entry;

#[allow(missing_docs)]
pub mod error;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
//#[derive(Clone, Deserialize, Serialize)]
#[derive(Clone)]
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
    StoreElement(ChainElement),
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
    StoreEntry(Signature, NewEntryHeader, Entry),
    /// Used to notify the authority for an agent's public key that that agent
    /// has commited a new header.
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
    RegisterReplacedBy(Signature, header::EntryUpdate, Entry),
    /// Op for deleting an entry
    RegisterDeletedBy(Signature, header::EntryDelete),
    /// Op for adding a link  
    RegisterAddLink(Signature, header::LinkAdd),
    /// Op for removing a link
    RegisterRemoveLink(Signature, header::LinkRemove),
}

impl DhtOp {
    /// Find the place to send this op
    pub fn dht_basis(&self) -> AnyDhtHash {
        match self {
            Self::StoreElement(ChainElement { signed_header, .. }) => {
                signed_header.header_address().clone().into()
            }
            Self::StoreEntry(_, header, _) => header.entry().clone().into(),
            Self::RegisterAgentActivity(_, header) => header.author().clone().into(),
            Self::RegisterReplacedBy(_, header, _) => header.replaces_address.clone().into(),
            Self::RegisterDeletedBy(_, header) => header.removes_address.clone(),
            Self::RegisterAddLink(_, header) => header.base_address.clone(),
            Self::RegisterRemoveLink(_, _header) => {
                todo!("LinkRemove header doesn't contain the base_address")
            }
        }
    }

    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::StoreElement(ChainElement { signed_header, .. }) => {
                UniqueForm::StoreElement(signed_header.header())
            }
            Self::StoreEntry(_, header, _) => UniqueForm::StoreEntry(header),
            Self::RegisterAgentActivity(_, header) => UniqueForm::RegisterAgentActivity(header),
            Self::RegisterReplacedBy(_, header, _) => UniqueForm::RegisterReplacedBy(header),
            Self::RegisterDeletedBy(_, header) => UniqueForm::RegisterDeletedBy(header),
            Self::RegisterAddLink(_, header) => UniqueForm::RegisterAddLink(header),
            Self::RegisterRemoveLink(_, header) => UniqueForm::RegisterRemoveLink(header),
        }
    }
}

/*
impl Hash for DhtOp {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.as_unique_form().hash(hasher)
    }
}

impl PartialEq for DhtOp {
    fn partial_eq(&self, other: &Self) -> Option<bool> {
        self.as_unique_form().partial_eq(other.as_unique_form())
    }
}
*/

//impl Eq for DhtOp {}

//#[derive(Eq, Hash, PartialEq)]
enum UniqueForm<'a> {
    // As an optimization, we don't include signatures. They would be redundant
    // with headers and therefore would waste hash/comparison time to include.
    StoreElement(&'a Header),
    StoreEntry(&'a NewEntryHeader),
    RegisterAgentActivity(&'a Header),
    // note: changed from entry to header since last discussion
    RegisterReplacedBy(&'a header::EntryUpdate),
    RegisterDeletedBy(&'a header::EntryDelete),

    // future work: encode idempotency in LinkAdd entries themselves
    RegisterAddLink(&'a header::LinkAdd),
    RegisterRemoveLink(&'a header::LinkRemove),
}

/// Turn a chain element into a DhtOp
pub fn ops_from_element(element: &ChainElement) -> DhtOpResult<Vec<DhtOp>> {
    // TODO: avoid cloning everything

    let (signed_header, maybe_entry) = element.clone().into_inner();
    let (header, sig) = signed_header.into_header_and_signature();
    let (header, _): (Header, _) = header.into();

    // TODO: avoid allocation, we have a static maximum of four items and
    // callers simply want to iterate over the ops.
    //
    // Maybe use `ArrayVec`?
    let mut ops = vec![
        DhtOp::StoreElement(element.clone()),
        DhtOp::RegisterAgentActivity(sig.clone(), header.clone()),
    ];

    match &header {
        Header::Dna(_)
        | Header::ChainOpen(_)
        | Header::ChainClose(_)
        | Header::AgentValidationPkg(_)
        | Header::InitZomesComplete(_) => {}
        Header::LinkAdd(link_add) => {
            ops.push(DhtOp::RegisterAddLink(sig.clone(), link_add.clone()))
        }
        Header::LinkRemove(link_remove) => {
            ops.push(DhtOp::RegisterRemoveLink(sig.clone(), link_remove.clone()))
        }
        Header::EntryCreate(header) => ops.push(DhtOp::StoreEntry(
            sig.clone(),
            NewEntryHeader::Create(header.clone()),
            maybe_entry
                .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.clone().into()))?
                .clone(),
        )),
        Header::EntryUpdate(entry_update) => {
            let entry = maybe_entry
                .ok_or_else(|| DhtOpError::HeaderWithoutEntry(entry_update.clone().into()))?
                .clone();
            ops.push(DhtOp::StoreEntry(
                sig.clone(),
                NewEntryHeader::Update(entry_update.clone()),
                entry.clone(),
            ));
            ops.push(DhtOp::RegisterReplacedBy(
                sig.clone(),
                entry_update.clone(),
                entry.clone(),
            ));
        }
        Header::EntryDelete(entry_delete) => {
            ops.push(DhtOp::RegisterDeletedBy(sig.clone(), entry_delete.clone()))
        }
    }
    Ok(ops)
}
