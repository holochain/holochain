//! Types needed for all validation

use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dht_op::UniqueForm;
use holochain_zome_types::element::{Element, SignedHeaderHashed};

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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
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
        self.pending.len() > 0
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
    pub fn store_entry_any(&mut self, dep: Dependency<Element>) -> Option<Element> {
        self.store_entry(dep, true)
    }

    /// A store entry dependency with a dependency on a specific element
    pub fn store_entry_fixed(&mut self, dep: Dependency<Element>) -> Option<Element> {
        self.store_entry(dep, false)
    }

    /// Create a dependency on a store entry op
    /// from an element.
    fn store_entry(&mut self, dep: Dependency<Element>, any: bool) -> Option<Element> {
        let el = match dep {
            Dependency::Claim(el) | Dependency::Proof(el) => el,
            Dependency::PendingValidation(el) => {
                let header = el.header().clone().try_into().ok()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreEntry(&header));
                if any {
                    self.pending.push(DepType::AnyElement(hash));
                } else {
                    self.pending.push(hash.into());
                }
                el
            }
        };
        Some(el)
    }

    /// Create a dependency on a store element op
    /// from a header.
    pub fn store_element(&mut self, dep: Dependency<SignedHeaderHashed>) -> SignedHeaderHashed {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreElement(header));
                self.pending.push(hash.into());
                shh
            }
        };
        shh
    }

    /// Create a dependency on a add link op
    /// from a header.
    pub fn add_link(&mut self, dep: Dependency<SignedHeaderHashed>) -> Option<SignedHeaderHashed> {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header = shh.header().clone().try_into().ok()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAddLink(&header));
                self.pending.push(hash.into());
                shh
            }
        };
        Some(shh)
    }

    /// Create a dependency on a register agent activity op
    /// from a header.
    pub fn register_agent_activity(
        &mut self,
        dep: Dependency<SignedHeaderHashed>,
    ) -> SignedHeaderHashed {
        let shh = match dep {
            Dependency::Claim(shh) | Dependency::Proof(shh) => shh,
            Dependency::PendingValidation(shh) => {
                let header = shh.header();
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAgentActivity(header));
                self.pending.push(hash.into());
                shh
            }
        };
        shh
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
