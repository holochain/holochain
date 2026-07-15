//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::error::DhtOpResult;
use crate::wire_ops::RenderedOp;
use crate::wire_ops::RenderedOps;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::*;
use holochain_zome_types::warrant::SignedWarrant;

/// Convenience function for when you have a RecordEntry but need
/// a Option EntryHashed
pub fn option_entry_hashed(entry: RecordEntry) -> Option<EntryHashed> {
    match entry {
        RecordEntry::Present(e) => Some(EntryHashed::from_content_sync(e)),
        _ => None,
    }
}

/// The record-serving response to a get-entry request.
///
/// Serves the create/delete/update actions on an entry plus the entry data,
/// each action carrying its record-level validation status. A `Rejected`
/// action is always accompanied by a warrant in `warrants` proving the
/// rejection; the receiver checks that invariant up front.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
pub struct WireEntryOps {
    /// Any actions that created this entry, each with its validation status.
    pub creates: Vec<Judged<SignedAction>>,
    /// Any deletes that deleted this entry, each with its validation status.
    pub deletes: Vec<Judged<SignedAction>>,
    /// Any updates on this entry, each with its validation status.
    pub updates: Vec<Judged<SignedAction>>,
    /// The entry data shared across all actions.
    pub entry: Option<EntryData>,
    /// Warrants proving any `Rejected` records served above.
    pub warrants: Vec<SignedWarrant>,
}

/// All entry data common to an get entry request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct EntryData {
    /// The entry shared across all actions.
    pub entry: Entry,
    /// The entry_type shared across all actions.
    pub entry_type: EntryType,
}

impl WireEntryOps {
    /// Create an empty wire response.
    pub fn new() -> Self {
        Self::default()
    }
    /// Expand the served records into the request-relevant ops for caching.
    ///
    /// Each served action becomes the single op the get-entry request
    /// represents (`CreateEntry` per create, `RegisterDeletedEntryAction` per
    /// delete, `RegisterUpdatedContent` per update), tagged with the served
    /// validation status. Warrants are handled separately by the requester.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            creates,
            deletes,
            updates,
            entry,
            warrants: _,
        } = self;
        match entry {
            Some(EntryData {
                entry,
                entry_type: _,
            }) => {
                let mut ops = Vec::with_capacity(creates.len() + deletes.len() + updates.len());
                let entry_hashed = EntryHashed::from_content_sync(entry);
                for op in creates {
                    let status = op.validation_status();
                    let (action, signature) = op.data.into();
                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        ChainOpType::CreateEntry,
                    )?);
                }
                for op in deletes {
                    let status = op.validation_status();
                    let (action, signature) = op.data.into();
                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        ChainOpType::DeleteEntry,
                    )?);
                }
                for op in updates {
                    let status = op.validation_status();
                    let (action, signature) = op.data.into();
                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        ChainOpType::UpdateEntry,
                    )?);
                }
                Ok(RenderedOps {
                    entry: Some(entry_hashed),
                    ops,
                    warrant: None,
                })
            }
            None => Ok(Default::default()),
        }
    }
}
