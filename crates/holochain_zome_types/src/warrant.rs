//! Types for warrants

use holo_hash::*;
use holochain_integrity_types::Signature;
pub use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

use crate::signature::Signed;

/// A Warrant is an authored, timestamped proof of wrongdoing by another agent.
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializedBytes,
    Eq,
    PartialEq,
    Hash,
    derive_more::From,
    derive_more::Deref,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary,)
)]
pub struct Warrant {
    /// The self-proving part of the warrant containing evidence of bad behavior
    #[deref]
    pub proof: WarrantProof,
    /// The author of the warrant
    pub author: AgentPubKey,
    /// Time when the warrant was issued
    pub timestamp: Timestamp,
}

impl Warrant {
    /// Constructor
    pub fn new(proof: WarrantProof, author: AgentPubKey, timestamp: Timestamp) -> Self {
        Self {
            proof,
            author,
            timestamp,
        }
    }

    /// Constructor with timestamp set to now()
    #[cfg(feature = "full")]
    pub fn new_now(proof: WarrantProof, author: AgentPubKey) -> Self {
        Self {
            proof,
            author,
            timestamp: Timestamp::now(),
        }
    }
}

impl HashableContent for Warrant {
    type HashType = holo_hash::hash_type::Warrant;

    fn hash_type(&self) -> Self::HashType {
        Self::HashType::new()
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(self.try_into().expect("Could not serialize Warrant"))
    }
}

/// The self-proving part of a Warrant which demonstrates bad behavior by another agent
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary,)
)]
pub enum WarrantProof {
    /// Signifies evidence of a breach of chain integrity
    ChainIntegrity(ChainIntegrityWarrant),
}

/// Just the type of the warrant
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    SerializedBytes,
    Eq,
    PartialEq,
    Hash,
    derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary,)
)]
pub enum WarrantType {
    // NOTE: the values here cannot overlap with ActionType,
    // because they occupy the same field in the Action table.
    //
    /// Signifies evidence of a breach of chain integrity
    ChainIntegrityWarrant,
}

impl From<Warrant> for WarrantType {
    fn from(warrant: Warrant) -> Self {
        warrant.get_type()
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
impl rusqlite::ToSql for WarrantType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            format!("{:?}", self).into(),
        ))
    }
}

/// A warrant which is sent to AgentActivity authorities
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum ChainIntegrityWarrant {
    /// Something invalid was authored on a chain.
    /// When we receive this warrant, we fetch the Action and validate it
    /// under every applicable DhtOpType.
    // TODO: include ChainOpType, which allows the receipient to only run
    //       validation for that op type. At the time of writing, this was
    //       non-trivial because ChainOpType is in a downstream crate.
    InvalidChainOp {
        /// The author of the action
        action_author: AgentPubKey,
        /// The hash of the action to fetch by
        action: ActionHashAndSig,
        /// Whether to run app or sys validation
        validation_type: ValidationType,
    },

    /// Proof of chain fork.
    ChainFork {
        /// Author of the chain which is forked
        chain_author: AgentPubKey,
        /// Two actions of the same seq number which prove the fork
        action_pair: (ActionHashAndSig, ActionHashAndSig),
    },
}

/// Action hash with the signature of the action at that hash
pub type ActionHashAndSig = (ActionHash, Signature);

impl WarrantProof {
    /// Basis hash where this warrant should be delivered.
    /// Warrants always have the authoring agent as a basis, so that warrants
    /// can be accumulated by the agent activity authorities.
    pub fn dht_basis(&self) -> OpBasis {
        self.action_author().clone().into()
    }

    /// The author of the action which led to this warrant, i.e. the target of the warrant
    pub fn action_author(&self) -> &AgentPubKey {
        match self {
            Self::ChainIntegrity(w) => match w {
                ChainIntegrityWarrant::InvalidChainOp { action_author, .. } => action_author,
                ChainIntegrityWarrant::ChainFork { chain_author, .. } => chain_author,
            },
        }
    }

    /// Get the warrant type
    pub fn get_type(&self) -> WarrantType {
        match self {
            Self::ChainIntegrity(_) => WarrantType::ChainIntegrityWarrant,
        }
    }
}

/// Not necessary but nice to have
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::Display,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum ValidationType {
    /// Sys validation
    Sys,
    /// App validation
    App,
}

/// A signed warrant with timestamp
pub type SignedWarrant = Signed<Warrant>;
