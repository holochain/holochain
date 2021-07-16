//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use holo_hash::*;
use holochain_zome_types::prelude::*;

use crate::dht_op::error::DhtOpResult;
use crate::dht_op::DhtOpType;
use crate::dht_op::RenderedOp;
use crate::dht_op::RenderedOps;
use crate::header::WireDelete;
use crate::header::WireNewEntryHeader;
use crate::header::WireUpdateRelationship;

/// Convenience function for when you have an ElementEntry but need
/// a Option EntryHashed
pub fn option_entry_hashed(entry: ElementEntry) -> Option<EntryHashed> {
    match entry {
        ElementEntry::Present(e) => Some(EntryHashed::from_content_sync(e)),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
/// Condensed data needed for a get entry request.
// TODO: Could use actual compression to get even smaller.
pub struct WireEntryOps {
    /// Any headers that created this entry.
    pub creates: Vec<Judged<WireNewEntryHeader>>,
    /// Any deletes that deleted this entry.
    // TODO: Can remove the entry hash from [`WireDelete`]
    // to save more data.
    pub deletes: Vec<Judged<WireDelete>>,
    /// Any updates on this entry.
    pub updates: Vec<Judged<WireUpdateRelationship>>,
    /// The entry data shared across all headers.
    pub entry: Option<EntryData>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
/// All entry data common to an get entry request.
pub struct EntryData {
    /// The entry shared across all headers.
    pub entry: Entry,
    /// The entry_type shared across all headers.
    pub entry_type: EntryType,
}

impl WireEntryOps {
    /// Create an empty wire response.
    pub fn new() -> Self {
        Self::default()
    }
    /// Render these ops to their full types.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            creates,
            deletes,
            updates,
            entry,
        } = self;
        match entry {
            Some(EntryData { entry, entry_type }) => {
                let mut ops = Vec::with_capacity(creates.len() + deletes.len() + updates.len());
                let entry_hashed = EntryHashed::from_content_sync(entry);
                for op in creates {
                    let status = op.validation_status();
                    let SignedHeader(header, signature) = op
                        .data
                        .into_signed_header(entry_type.clone(), entry_hashed.as_hash().clone());

                    ops.push(RenderedOp::new(
                        header,
                        signature,
                        status,
                        DhtOpType::StoreEntry,
                    )?);
                }
                for op in deletes {
                    let status = op.validation_status();
                    let op = op.data;
                    let signature = op.signature;
                    let header = Header::Delete(op.delete);

                    ops.push(RenderedOp::new(
                        header,
                        signature,
                        status,
                        DhtOpType::RegisterDeletedEntryHeader,
                    )?);
                }
                for op in updates {
                    let status = op.validation_status();
                    let SignedHeader(header, signature) =
                        op.data.into_signed_header(entry_hashed.as_hash().clone());

                    ops.push(RenderedOp::new(
                        header,
                        signature,
                        status,
                        DhtOpType::RegisterUpdatedContent,
                    )?);
                }
                Ok(RenderedOps {
                    entry: Some(entry_hashed),
                    ops,
                })
            }
            None => Ok(Default::default()),
        }
    }
}
