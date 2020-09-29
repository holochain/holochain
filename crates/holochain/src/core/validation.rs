//! Types needed for all validation
use derivative::Derivative;
use holo_hash::DhtOpHash;
use holochain_types::dht_op::DhtOp;

/// Sets the level required for validation dependencies
#[derive(Clone, Debug, Copy)]
pub enum CheckLevel {
    /// Selected dependencies must be held by this agent
    Hold,
    /// Selected dependencies must be witnessed by this agent
    Witness,
}

/// Exit early with either an outcome or an error
pub enum OutcomeOrError<T, E> {
    Outcome(T),
    Err(E),
}

/// Helper macro for implementing from sub error types
/// for the error in OutcomeOrError
#[macro_export]
macro_rules! from_sub_error {
    ($error_type:ident, $sub_error_type:ident) => {
        impl<T> From<$sub_error_type> for OutcomeOrError<T, $error_type> {
            fn from(e: $sub_error_type) -> Self {
                OutcomeOrError::Err($error_type::from(e))
            }
        }
    };
}

/// Type for deriving ordering of DhtOps
/// Don't change the order of this enum unless
/// you mean to change the order we process ops
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DhtOpOrder {
    RegisterAgentActivity(holochain_zome_types::timestamp::Timestamp),
    StoreEntry(holochain_zome_types::timestamp::Timestamp),
    StoreElement(holochain_zome_types::timestamp::Timestamp),
    RegisterUpdatedBy(holochain_zome_types::timestamp::Timestamp),
    RegisterDeletedBy(holochain_zome_types::timestamp::Timestamp),
    RegisterDeletedEntryHeader(holochain_zome_types::timestamp::Timestamp),
    RegisterAddLink(holochain_zome_types::timestamp::Timestamp),
    RegisterRemoveLink(holochain_zome_types::timestamp::Timestamp),
}

/// Op data that will be ordered by [DhtOpOrder]
#[derive(Derivative, Debug, Clone)]
#[derivative(Eq, PartialEq, Ord, PartialOrd)]
pub struct OrderedOp<V> {
    pub order: DhtOpOrder,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    pub hash: DhtOpHash,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    pub op: DhtOp,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    pub value: V,
}

impl From<&DhtOp> for DhtOpOrder {
    fn from(op: &DhtOp) -> Self {
        use DhtOpOrder::*;
        match op {
            DhtOp::StoreElement(_, h, _) => StoreElement(h.timestamp()),
            DhtOp::StoreEntry(_, h, _) => StoreEntry(*h.timestamp()),
            DhtOp::RegisterAgentActivity(_, h) => RegisterAgentActivity(h.timestamp()),
            DhtOp::RegisterUpdatedBy(_, h, _) => RegisterUpdatedBy(h.timestamp),
            DhtOp::RegisterDeletedBy(_, h) => RegisterDeletedBy(h.timestamp),
            DhtOp::RegisterDeletedEntryHeader(_, h) => RegisterDeletedEntryHeader(h.timestamp),
            DhtOp::RegisterAddLink(_, h) => RegisterAddLink(h.timestamp),
            DhtOp::RegisterRemoveLink(_, h) => RegisterRemoveLink(h.timestamp),
        }
    }
}
