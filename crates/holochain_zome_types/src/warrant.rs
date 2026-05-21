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
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            format!("{self:?}").into(),
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

/// Maximum byte length of the human-readable reason carried in
/// [`ChainIntegrityWarrant::InvalidChainOp`]. Warrants exceeding this
/// limit must be rejected by sys validation to prevent griefing via
/// oversized warrant payloads.
pub const MAX_WARRANT_REASON_BYTES: usize = 512;

/// Truncate `s` so that its UTF-8 byte length does not exceed
/// `MAX_WARRANT_REASON_BYTES`, while preserving UTF-8 char boundaries.
pub fn truncate_warrant_reason(s: &str) -> String {
    let end = s.floor_char_boundary(MAX_WARRANT_REASON_BYTES);
    s[..end].to_string()
}

/// A warrant which is sent to agent activity authorities.
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
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
        /// A human readable reason for why this action is invalid.
        ///
        /// This field is not considered for equality or hashing, since it's just a human-readable
        /// explanation and should not affect the identity of the warrant.
        reason: String,
    },

    /// Proof of a chain fork.
    ChainFork {
        /// Author of the chain which is forked
        chain_author: AgentPubKey,
        /// Two actions of the same seq number which prove the fork
        action_pair: (ActionHashAndSig, ActionHashAndSig),
        /// The seq number at which the fork occurs
        seq: u32,
    },
}

impl PartialEq for ChainIntegrityWarrant {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                ChainIntegrityWarrant::InvalidChainOp {
                    action_author: a1,
                    action: a2,
                    chain_op_type: a3,
                    reason: _, // reason is not considered for equality
                },
                ChainIntegrityWarrant::InvalidChainOp {
                    action_author: b1,
                    action: b2,
                    chain_op_type: b3,
                    reason: _, // reason is not considered for equality
                },
            ) => a1 == b1 && a2 == b2 && a3 == b3,
            (
                ChainIntegrityWarrant::ChainFork {
                    chain_author: a1,
                    action_pair: a2,
                    seq: a3,
                },
                ChainIntegrityWarrant::ChainFork {
                    chain_author: b1,
                    action_pair: b2,
                    seq: b3,
                },
            ) => a1 == b1 && a2 == b2 && a3 == b3,
            _ => false,
        }
    }
}

impl Eq for ChainIntegrityWarrant {}

impl std::hash::Hash for ChainIntegrityWarrant {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ChainIntegrityWarrant::InvalidChainOp {
                action_author,
                action,
                chain_op_type,
                reason: _, // reason excluded from hash since it's not considered for equality
            } => {
                0u8.hash(state);
                action_author.hash(state);
                action.hash(state);
                chain_op_type.hash(state);
            }
            ChainIntegrityWarrant::ChainFork {
                chain_author,
                action_pair,
                seq,
            } => {
                1u8.hash(state);
                chain_author.hash(state);
                action_pair.hash(state);
                seq.hash(state);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid_chain_op(reason: &str) -> ChainIntegrityWarrant {
        ChainIntegrityWarrant::InvalidChainOp {
            action_author: AgentPubKey::from_raw_36(vec![1u8; 36]),
            action: (
                ActionHash::from_raw_36(vec![2u8; 36]),
                Signature::from([3u8; 64]),
            ),
            chain_op_type: ChainOpType::StoreRecord,
            reason: reason.to_string(),
        }
    }

    #[test]
    fn invalid_chain_op_eq_ignores_reason() {
        let a = invalid_chain_op("first reason");
        let b = invalid_chain_op("totally different reason");
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_chain_op_eq_checks_other_fields() {
        let a = invalid_chain_op("r");
        let mut b = invalid_chain_op("r");
        if let ChainIntegrityWarrant::InvalidChainOp { chain_op_type, .. } = &mut b {
            *chain_op_type = ChainOpType::StoreEntry;
        }
        assert_ne!(a, b);
    }

    #[test]
    fn truncate_warrant_reason_under_limit_unchanged() {
        let s = "short reason".to_string();
        assert_eq!(truncate_warrant_reason(&s), s);
    }

    #[test]
    fn truncate_warrant_reason_at_limit_unchanged() {
        let s = "a".repeat(MAX_WARRANT_REASON_BYTES);
        assert_eq!(truncate_warrant_reason(&s).len(), s.len());
    }

    #[test]
    fn truncate_warrant_reason_over_limit_clipped() {
        let s = "a".repeat(MAX_WARRANT_REASON_BYTES + 100);
        let out = truncate_warrant_reason(&s);
        assert_eq!(out.len(), MAX_WARRANT_REASON_BYTES);
    }

    #[test]
    fn truncate_warrant_reason_respects_char_boundaries() {
        // Each "€" is 3 bytes in UTF-8. With a 512-byte limit, 171 chars occupy
        // 513 bytes — the limit at byte 512 lands in the middle of the last
        // codepoint (which starts at byte 510), forcing the boundary-backoff
        // loop to retreat to 510.
        let char_count = MAX_WARRANT_REASON_BYTES / 3 + 1;
        let s: String = "€".repeat(char_count);
        assert!(s.len() > MAX_WARRANT_REASON_BYTES);
        assert!(!s.is_char_boundary(MAX_WARRANT_REASON_BYTES));
        let out = truncate_warrant_reason(&s);
        // Truncation must back off below the limit to land on a boundary.
        assert!(out.len() < MAX_WARRANT_REASON_BYTES);
        assert_eq!(out.len() % 3, 0);
        assert!(out.is_char_boundary(out.len()));
        // Should still be valid UTF-8 (`String` guarantees this, but explicit check):
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn invalid_chain_op_hash_ignores_reason() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a = invalid_chain_op("first reason");
        let b = invalid_chain_op("totally different reason");
        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn chain_fork_ne_invalid_chain_op() {
        let invalid = invalid_chain_op("r");
        let fork = ChainIntegrityWarrant::ChainFork {
            chain_author: AgentPubKey::from_raw_36(vec![1u8; 36]),
            action_pair: (
                (
                    ActionHash::from_raw_36(vec![2u8; 36]),
                    Signature::from([3u8; 64]),
                ),
                (
                    ActionHash::from_raw_36(vec![4u8; 36]),
                    Signature::from([5u8; 64]),
                ),
            ),
            seq: 0,
        };
        assert_ne!(invalid, fork);
    }
}
