//! Defines a Record, the basic unit of Holochain data.

use crate::prelude::*;
use holochain_zome_types::prelude::{EntryHashed, SignedWarrant};

/// The record-serving response to a get-record request.
///
/// Serves the requested record as actions plus its entry (when public), each
/// action carrying its record-level validation status. A `Rejected` action is
/// always accompanied by a warrant in `warrants` proving the rejection; the
/// receiver checks that invariant up front before doing any validation work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
pub struct WireRecordOps {
    /// The action this request was for, with its validation status.
    pub action: Option<Judged<SignedAction>>,
    /// Any deletes on the action, each with its validation status.
    pub deletes: Vec<Judged<SignedAction>>,
    /// Any updates on the action, each with its validation status.
    pub updates: Vec<Judged<SignedAction>>,
    /// The entry if there is one.
    pub entry: Option<Entry>,
    /// Warrants proving any `Rejected` records served above.
    pub warrants: Vec<SignedWarrant>,
}

impl WireRecordOps {
    /// Create an empty set of wire record ops.
    pub fn new() -> Self {
        Self::default()
    }
    /// Expand the served records into the request-relevant ops for caching.
    ///
    /// Each served action becomes the single op the get-record request
    /// represents (`CreateRecord` for the record itself, `DeleteRecord`
    /// per delete, `UpdateRecord` per update), tagged with the served
    /// validation status. Warrants are handled separately by the requester.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            action,
            deletes,
            updates,
            entry,
            warrants: _,
        } = self;
        let mut ops = Vec::with_capacity(1 + deletes.len() + updates.len());
        if let Some(action) = action {
            let status = action.validation_status();
            let (action, signature) = action.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::CreateRecord,
            )?);
        }
        for op in deletes {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::DeleteRecord,
            )?);
        }
        for op in updates {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::UpdateRecord,
            )?);
        }
        Ok(RenderedOps {
            entry: entry.map(EntryHashed::from_content_sync),
            ops,
            warrant: None,
        })
    }
}

/// Record with it's status
#[derive(Debug, Clone, derive_more::Constructor)]
pub struct RecordStatus {
    /// The record this status applies to.
    pub record: Record,
    /// Validation status of this record.
    pub status: ValidationStatus,
}
