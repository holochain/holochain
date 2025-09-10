//! Types for warrants

use crate::op::ChainOpType;
use crate::signature::Signed;
use holo_hash::*;
use holochain_integrity_types::Signature;
pub use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;
#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
use {
    rusqlite::{
        types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
        ToSql,
    },
    std::str::FromStr,
};

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
pub struct Warrant {
    /// The self-proving part of the warrant containing evidence of bad behavior.
    #[deref]
    pub proof: WarrantProof,
    /// The author of the warrant.
    pub author: AgentPubKey,
    /// Time when the warrant was issued.
    pub timestamp: Timestamp,
    /// The warranted agent.
    pub warrantee: AgentPubKey,
}

impl Warrant {
    /// Constructor
    pub fn new(
        proof: WarrantProof,
        author: AgentPubKey,
        timestamp: Timestamp,
        warrantee: AgentPubKey,
    ) -> Self {
        Self {
            proof,
            author,
            timestamp,
            warrantee,
        }
    }

    /// Constructor with timestamp set to now()
    #[cfg(feature = "full")]
    pub fn new_now(proof: WarrantProof, author: AgentPubKey, warrantee: AgentPubKey) -> Self {
        Self {
            proof,
            author,
            timestamp: Timestamp::now(),
            warrantee,
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
pub enum WarrantProof {
    /// Signifies evidence of a breach of chain integrity.
    ChainIntegrity(ChainIntegrityWarrant),
}

/// The type of warrant.
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
    strum_macros::EnumString,
)]
pub enum WarrantType {
    /// Signifies evidence of a breach of chain integrity
    ChainIntegrityWarrant,
}

impl From<Warrant> for WarrantType {
    fn from(warrant: Warrant) -> Self {
        warrant.get_type()
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
impl ToSql for WarrantType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            format!("{:?}", self).into(),
        ))
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
impl FromSql for WarrantType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value)
            .and_then(|text| WarrantType::from_str(&text).map_err(|_| FromSqlError::InvalidType))
    }
}

/// A warrant which is sent to agent activity authorities.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash)]
pub enum ChainIntegrityWarrant {
    /// Something invalid was authored on a chain.
    ///
    /// When we receive this warrant, we fetch the Action and validate it as the specified chain
    /// op type.
    InvalidChainOp {
        /// The author of the invalid action.
        action_author: AgentPubKey,
        /// The action hash and its signature.
        action: ActionHashAndSig,
        /// The chain op type that was the validation context for this action being judged invalid.
        chain_op_type: ChainOpType,
    },

    /// Proof of a chain fork.
    ChainFork {
        /// Author of the chain which is forked
        chain_author: AgentPubKey,
        /// Two actions of the same seq number which prove the fork
        action_pair: (ActionHashAndSig, ActionHashAndSig),
    },
}

/// Action hash with the signature of the action at that hash.
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

/// A signed warrant with a timestamp
pub type SignedWarrant = Signed<Warrant>;
