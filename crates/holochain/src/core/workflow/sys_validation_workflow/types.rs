use super::*;

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
    RegisterAgentActivity,
    StoreEntry,
    StoreElement,
    RegisterUpdatedBy,
    RegisterDeletedBy,
    RegisterDeletedEntryHeader,
    RegisterAddLink,
    RegisterRemoveLink,
}

impl From<&DhtOp> for DhtOpOrder {
    fn from(op: &DhtOp) -> Self {
        use DhtOpOrder::*;
        match op {
            DhtOp::StoreElement(_, _, _) => StoreElement,
            DhtOp::StoreEntry(_, _, _) => StoreEntry,
            DhtOp::RegisterAgentActivity(_, _) => RegisterAgentActivity,
            DhtOp::RegisterUpdatedBy(_, _) => RegisterUpdatedBy,
            DhtOp::RegisterDeletedBy(_, _) => RegisterDeletedBy,
            DhtOp::RegisterDeletedEntryHeader(_, _) => RegisterDeletedEntryHeader,
            DhtOp::RegisterAddLink(_, _) => RegisterAddLink,
            DhtOp::RegisterRemoveLink(_, _) => RegisterRemoveLink,
        }
    }
}
