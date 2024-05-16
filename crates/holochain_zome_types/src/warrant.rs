//! Types for warrants

use holo_hash::*;
use holochain_integrity_types::Signature;
pub use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

use crate::signature::Signed;

/// Placeholder for warrant type
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum Warrant {
    /// Signifies evidence of a breach of chain integrity
    ChainIntegrity(ChainIntegrityWarrant),
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

impl Warrant {
    /// Basis hash where this warrant should be delivered.
    /// Warrants always have the authoring agent as a basis, so that warrants
    /// can be accumulated by the agent activity authorities.
    pub fn dht_basis(&self) -> OpBasis {
        match self {
            Warrant::ChainIntegrity(w) => match w {
                ChainIntegrityWarrant::InvalidChainOp { action_author, .. } => {
                    action_author.clone().into()
                }
                ChainIntegrityWarrant::ChainFork { chain_author, .. } => {
                    chain_author.clone().into()
                }
            },
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
pub type SignedWarrant = Signed<(Warrant, Timestamp)>;
