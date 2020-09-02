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

pub enum Dependency<T> {
    /// This agent is holding this dependency and it has passed validation
    Proof(T),
    /// Another agent is holding this dependency and is claiming they ran validation
    Claim(T),
    /// This agent is has this dependency but has not passed validation
    AwaitingProof(T),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Dependencies {
    pub deps: Vec<DepType>,
}

/// Dependencies can either be fixed to a specific element or
/// any element with the same entry. This changes how we handle
/// dependencies that turn out to be invalid.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum DepType {
    Fixed(DhtOpHash),
    Any(DhtOpHash),
}

/// Sets the level required for validation
#[derive(Clone, Debug, Copy)]
pub enum CheckLevel {
    /// Selected dependencies must be held by this agent
    Holding,
    /// Selected dependencies must be held by another authority
    Dht,
}

impl<T> Dependency<T> {
    /// Change this dep to the minimum of the two.
    /// Lowest to highest: AwaitingProof, Claim, Proof
    pub fn min<A>(self, other: &Dependency<A>) -> Dependency<T> {
        // Dependency::Proof(_) => Dependency::Proof(self.into_inner()),
        use Dependency::*;
        match (&self, other) {
            (Proof(_), Proof(_)) => Proof(self.into_inner()),
            (Proof(_), Claim(_)) => Claim(self.into_inner()),
            (Proof(_), AwaitingProof(_)) => AwaitingProof(self.into_inner()),
            (Claim(_), Proof(_)) => Claim(self.into_inner()),
            (Claim(_), Claim(_)) => Claim(self.into_inner()),
            (Claim(_), AwaitingProof(_)) => AwaitingProof(self.into_inner()),
            (AwaitingProof(_), Proof(_)) => AwaitingProof(self.into_inner()),
            (AwaitingProof(_), Claim(_)) => AwaitingProof(self.into_inner()),
            (AwaitingProof(_), AwaitingProof(_)) => AwaitingProof(self.into_inner()),
        }
    }

    pub fn into_inner(self) -> T {
        match self {
            Dependency::Proof(t) | Dependency::Claim(t) | Dependency::AwaitingProof(t) => t,
        }
    }

    pub fn as_inner(&self) -> &T {
        match self {
            Dependency::Proof(t) | Dependency::Claim(t) | Dependency::AwaitingProof(t) => t,
        }
    }
}

impl Dependencies {
    pub fn new() -> Self {
        Self { deps: Vec::new() }
    }
    pub fn awaiting_proof(&self) -> bool {
        self.deps.len() > 0
    }
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
    async fn store_entry(
        &mut self,
        dep: Dependency<Element>,
        any: bool,
    ) -> SysValidationResult<Element> {
        let el = match dep {
            Dependency::Proof(el) => el,
            Dependency::AwaitingProof(el) => {
                let header = el
                    .header()
                    .clone()
                    .try_into()
                    .map_err(|_| ValidationError::NotNewEntry(el.header().clone()))?;
                let hash = DhtOpHash::with_data(&UniqueForm::StoreEntry(&header)).await;
                if any {
                    self.deps.push(DepType::Any(hash));
                } else {
                    self.deps.push(hash.into());
                }
                el
            }
            Dependency::Claim(el) => el,
        };
        Ok(el)
    }
    pub async fn store_element(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Proof(shh) => shh,
            Dependency::AwaitingProof(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data(&UniqueForm::StoreElement(header)).await;
                self.deps.push(hash.into());
                shh
            }
            Dependency::Claim(shh) => shh,
        };
        Ok(shh)
    }

    pub async fn add_link(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Proof(shh) => shh,
            Dependency::AwaitingProof(shh) => {
                let header = shh
                    .header()
                    .clone()
                    .try_into()
                    .map_err(|_| ValidationError::NotLinkAdd(shh.header_address().clone()))?;
                let hash = DhtOpHash::with_data(&UniqueForm::RegisterAddLink(&header)).await;
                self.deps.push(hash.into());
                shh
            }
            Dependency::Claim(shh) => shh,
        };
        Ok(shh)
    }

    pub async fn register_agent_activity(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SysValidationResult<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Proof(shh) => shh,
            Dependency::AwaitingProof(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data(&UniqueForm::RegisterAgentActivity(header)).await;
                self.deps.push(hash.into());
                shh
            }
            Dependency::Claim(shh) => shh,
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
            DepType::Fixed(h) | DepType::Any(h) => h,
        }
    }
}

impl From<DhtOpHash> for DepType {
    /// Creates a fixed dependency type
    fn from(d: DhtOpHash) -> Self {
        DepType::Fixed(d)
    }
}

impl AsRef<DhtOpHash> for DepType {
    fn as_ref(&self) -> &DhtOpHash {
        match self {
            DepType::Fixed(h) | DepType::Any(h) => h,
        }
    }
}
