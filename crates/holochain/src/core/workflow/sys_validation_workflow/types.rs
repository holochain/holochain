use super::*;
use derivative::Derivative;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dht_op::UniqueForm;
use holochain_zome_types::element::SignedHeaderHashed;

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

/// Ops have dependencies when validating that can be found from
/// different sources.
/// It is useful too know where a dependency is from because
/// we might need to wait for it to be validated or we might
/// need a stronger form of dependency.
pub enum Dependency<T> {
    /// This agent is holding this dependency and it has passed validation
    Proof(T),
    /// Another agent is holding this dependency and is claiming they ran validation
    Claim(T),
    /// This agent is has this dependency but has not passed validation yet
    PendingValidation(T),
}

/// PendingDependencies can either be fixed to a specific element or
/// any element with the same entry. This changes how we handle
/// dependencies that turn out to be invalid.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub enum DepType {
    /// The dependency is a specific element
    FixedElement(DhtOpHash),
    /// The dependency is any element for an entry
    AnyElement(DhtOpHash),
}

/// Sets the level required for validation dependencies
#[derive(Clone, Debug, Copy)]
pub enum CheckLevel {
    /// Selected dependencies must be validated by this agent
    Proof,
    /// Selected dependencies must be validated by another authority
    Claim,
}

impl<T> Dependency<T> {
    /// Change this dep to the minimum of the two.
    /// Lowest to highest: PendingValidation, Claim, Proof.
    /// Useful when you are chaining related dependencies together
    /// and need to treat them as the least strong source of the set.
    pub fn min<A>(self, other: &Dependency<A>) -> Dependency<T> {
        use Dependency::*;
        match (&self, other) {
            (Proof(_), Proof(_)) => Proof(self.into_inner()),
            (Proof(_), Claim(_)) => Claim(self.into_inner()),
            (Proof(_), PendingValidation(_)) => PendingValidation(self.into_inner()),
            (Claim(_), Proof(_)) => Claim(self.into_inner()),
            (Claim(_), Claim(_)) => Claim(self.into_inner()),
            (Claim(_), PendingValidation(_)) => PendingValidation(self.into_inner()),
            (PendingValidation(_), Proof(_)) => PendingValidation(self.into_inner()),
            (PendingValidation(_), Claim(_)) => PendingValidation(self.into_inner()),
            (PendingValidation(_), PendingValidation(_)) => PendingValidation(self.into_inner()),
        }
    }

    pub fn into_inner(self) -> T {
        match self {
            Dependency::Proof(t) | Dependency::Claim(t) | Dependency::PendingValidation(t) => t,
        }
    }

    pub fn as_inner(&self) -> &T {
        match self {
            Dependency::Proof(t) | Dependency::Claim(t) | Dependency::PendingValidation(t) => t,
        }
    }
}

/// This type allows ops to be optimistically validated using dependencies
/// that are in the limbo but have not themselves passed validation yet.
/// ## Example
/// Op A needs a Header (DH) to validate.
/// Op B contains a copy of this header (B-DH)
/// At the moment when Op A is looking for DH it finds B-DH.
/// At this same moment Op B has not finished validating so therefor
/// B-DH is still pending validation.
/// Op A can still use B-DH as the Header to continue it's own validation.
/// Now both Op A and Op B are running though validation.
/// However because Op A has B-DH as a PendingDependency it must
/// recheck if Op B has finished validating before Op A can be
/// considered valid.
///
/// ## Failed validation
/// If in the above scenario Op B fails to validate then
/// Op A will also fail validation.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct PendingDependencies {
    /// PendingDependencies that hadn't finished validation at the
    /// time we used them to validate this op.
    pub pending: Vec<DepType>,
}

impl PendingDependencies {
    /// Create a new pending deps
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Are there any dependencies that we need to check?
    pub fn pending_dependencies(&self) -> bool {
        !self.pending.is_empty()
    }
}

impl Default for PendingDependencies {
    fn default() -> Self {
        Self::new()
    }
}

/// ## Helpers
/// These functions help create the DhtOpHash
/// for the type DhtOp that you need to await for.
/// ## Dimensions
/// There are two dimensions to a dependency:
/// 1. The type of DhtOp (e.g. StoreElement or StoreEntry)
/// 2. If you require s specific (fixed) element or
/// any element for an entry. See [DepType]
///
/// The functions here are intended to make it simple
/// to go from a dependency that you have found to it's
/// inner type whilst recording the correct DhtOp and DepType
/// to check when the op reaches the end of validation
impl PendingDependencies {
    /// A store entry dependency where you don't care which element was found
    pub async fn store_entry_any(
        &mut self,
        dep: Dependency<Element>,
    ) -> SysValidationResult<Element> {
        self.store_entry(dep, true).await
    }

    /// A store entry dependency with a dependency on a specific element
    pub async fn store_entry_fixed(
        &mut self,
        dep: Dependency<Element>,
    ) -> SysValidationResult<Element> {
        self.store_entry(dep, false).await
    }

    /// Create a dependency on a store entry op
    /// from an element.
    async fn store_entry(
        &mut self,
        dep: Dependency<Element>,
        any: bool,
    ) -> SysValidationResult<Element> {
        let el = match dep {
            Dependency::Claim(el) | Dependency::Proof(el) => el,
            Dependency::PendingValidation(el) => {
                let header = el
                    .header()
                    .clone()
                    .try_into()
                    .map_err(|_| ValidationOutcome::NotNewEntry(el.header().clone()))?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreEntry(&header));
                if any {
                    self.pending.push(DepType::AnyElement(hash));
                } else {
                    self.pending.push(hash.into());
                }
                el
            }
        };
        Ok(el)
    }

    /// Create a dependency on a store element op
    /// from a header.
    pub async fn store_element(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreElement(header));
                self.pending.push(hash.into());
                shh
            }
        };
        Ok(shh)
    }

    /// Create a dependency on a add link op
    /// from a header.
    pub async fn add_link(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header =
                    shh.header().clone().try_into().map_err(|_| {
                        ValidationOutcome::NotCreateLink(shh.header_address().clone())
                    })?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAddLink(&header));
                self.pending.push(hash.into());
                shh
            }
        };
        Ok(shh)
    }

    /// Create a dependency on a register agent activity op
    /// from a header.
    pub async fn register_agent_activity(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAgentActivity(header));
                self.pending.push(hash.into());
                shh
            }
        };
        Ok(shh)
    }
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

impl From<DepType> for DhtOpHash {
    fn from(d: DepType) -> Self {
        match d {
            DepType::FixedElement(h) | DepType::AnyElement(h) => h,
        }
    }
}

impl From<DhtOpHash> for DepType {
    /// Creates a fixed dependency type
    fn from(d: DhtOpHash) -> Self {
        DepType::FixedElement(d)
    }
}

impl AsRef<DhtOpHash> for DepType {
    fn as_ref(&self) -> &DhtOpHash {
        match self {
            DepType::FixedElement(h) | DepType::AnyElement(h) => h,
        }
    }
}
