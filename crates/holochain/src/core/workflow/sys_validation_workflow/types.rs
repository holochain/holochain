use super::*;
use derivative::Derivative;

#[derive(Debug)]
/// The outcome of sys validation
pub(super) enum Outcome {
    /// Moves to app validation
    Accepted,
    /// Moves straight to integration
    SkipAppValidation,
    /// Stays in limbo because another DhtOp
    /// dependency needs to be validated first
    AwaitingOpDep(AnyDhtHash),
    /// Stays in limbo because a dependency could not
    /// be found currently on the DHT.
    /// Note this is not proof it doesn't exist.
    MissingDhtDep,
    /// Moves to integration with status rejected
    Rejected,
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
            DhtOp::RegisterUpdatedBy(_, h) => RegisterUpdatedBy(h.timestamp),
            DhtOp::RegisterDeletedBy(_, h) => RegisterDeletedBy(h.timestamp),
            DhtOp::RegisterDeletedEntryHeader(_, h) => RegisterDeletedEntryHeader(h.timestamp),
            DhtOp::RegisterAddLink(_, h) => RegisterAddLink(h.timestamp),
            DhtOp::RegisterRemoveLink(_, h) => RegisterRemoveLink(h.timestamp),
        }
    }
}
