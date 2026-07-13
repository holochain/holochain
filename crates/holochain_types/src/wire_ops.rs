//! Wire representations of DHT ops served in response to GET requests, and the
//! rendered form they expand into for caching.

use crate::dht_v2::HashedChainOp;
use crate::error::{DhtOpError, DhtOpResult};
use crate::warrant::WarrantOp;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::*;

/// Condensed version of ops for sending across the wire.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum WireOps {
    /// Response for get entry.
    Entry(crate::entry::WireEntryOps),
    /// Response for get record.
    Record(crate::record::WireRecordOps),
    /// A warrant in place of data in the case that the data is invalid.
    /// There is no "wire" version because this is about as compact as it gets.
    Warrant(Box<WarrantOp>),
}

impl WireOps {
    /// Render the wire ops to DhtOps.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        match self {
            WireOps::Entry(o) => o.render(),
            WireOps::Record(o) => o.render(),
            WireOps::Warrant(warrant) => Ok(RenderedOps {
                entry: Default::default(),
                ops: Default::default(),
                warrant: Some(*warrant),
            }),
        }
    }

    /// The warrants accompanying this response, proving any `Rejected` records
    /// served. Empty for the standalone-warrant variant.
    pub fn warrants(&self) -> &[holochain_zome_types::warrant::SignedWarrant] {
        match self {
            WireOps::Entry(o) => &o.warrants,
            WireOps::Record(o) => &o.warrants,
            WireOps::Warrant(_) => &[],
        }
    }
}

/// The data rendered from a wire op to place in the database.
///
/// Wraps a [`HashedChainOp`] (the op with all hashes, basis, and storage
/// location pre-computed) alongside the served record-level validation status.
/// Dereferences to the wrapped [`HashedChainOp`] so callers can read its op
/// hash, signed action, op type, and basis directly.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RenderedOp {
    /// The op with its pre-computed hashes and basis.
    pub op: HashedChainOp,
    /// The validation status served for this op.
    pub validation_status: Option<ValidationStatus>,
}

impl std::ops::Deref for RenderedOp {
    type Target = HashedChainOp;

    fn deref(&self) -> &Self::Target {
        &self.op
    }
}

impl RenderedOp {
    /// Create a new rendered op from a wire's action.
    ///
    /// Computes the op hash, DHT basis, and storage location from the action.
    /// The entry (if any) is carried separately on the parent [`RenderedOps`],
    /// so it is not attached here.
    pub fn new(
        action: holochain_zome_types::dht_v2::Action,
        signature: Signature,
        validation_status: Option<ValidationStatus>,
        op_type: ChainOpType,
    ) -> DhtOpResult<Self> {
        let action_hashed = holo_hash::HoloHashed::from_content_sync(action);
        let signed_action = holochain_zome_types::dht_v2::SignedActionHashed::with_presigned(
            action_hashed,
            signature,
        );
        let op = HashedChainOp::from_signed_action(signed_action, op_type, None)
            .ok_or(DhtOpError::OpTypeActionMismatch(op_type))?;
        Ok(Self {
            op,
            validation_status,
        })
    }
}

/// The full data for insertion into the database.
/// The reason we don't use `DhtOp` is because we don't
/// want to clone the entry for every action.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RenderedOps {
    /// Entry for the ops if there is one.
    pub entry: Option<EntryHashed>,
    /// Op data to insert.
    pub ops: Vec<RenderedOp>,
    /// Warrant, if the data is invalid.
    /// If this is Some, all other fields should be empty, and vice versa.
    // TODO: RenderedOps really should be an enum, for the valid and invalid cases.
    pub warrant: Option<WarrantOp>,
}
