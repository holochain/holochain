use super::*;

use derive_more::Display;
use thiserror::Error;

/// Abstraction over an item in a chain.
// Alternate implementations are only used for testing, so this should not
// add a large monomorphization overhead
pub trait ChainItem: Clone + PartialEq + Eq + std::fmt::Debug + Send + Sync {
    /// The type used to represent a hash of this item
    type Hash: Into<ActionHash>
        + Clone
        + PartialEq
        + Eq
        + Ord
        + std::hash::Hash
        + std::fmt::Debug
        + Send
        + Sync;

    /// The sequence in the chain
    fn seq(&self) -> u32;

    /// The hash of this item
    fn get_hash(&self) -> &Self::Hash;

    /// The hash of the previous item
    fn prev_hash(&self) -> Option<&Self::Hash>;

    /// A display representation of the item
    fn to_display(&self) -> String;
}

/// Alias for getting the associated hash type of a ChainItem
pub type ChainItemHash<I> = <I as ChainItem>::Hash;

impl ChainItem for ActionHashed {
    type Hash = ActionHash;

    fn seq(&self) -> u32 {
        self.action_seq()
    }

    fn get_hash(&self) -> &Self::Hash {
        self.as_hash()
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.prev_action()
    }

    fn to_display(&self) -> String {
        format!("{}", self.content)
    }
}

impl ChainItem for SignedActionHashed {
    type Hash = ActionHash;

    fn seq(&self) -> u32 {
        self.hashed.seq()
    }

    fn get_hash(&self) -> &Self::Hash {
        self.hashed.get_hash()
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.hashed.prev_hash()
    }

    fn to_display(&self) -> String {
        format!("{}", self.hashed.content)
    }
}

/// Validate that a sequence of actions forms a valid hash chain via `prev_action`,
/// with an optional starting point.
pub fn validate_chain<'iter, A: 'iter + ChainItem>(
    mut actions: impl Iterator<Item = &'iter A>,
    persisted_chain_head: &Option<(A::Hash, u32)>,
) -> PrevActionResult<()> {
    // Check the chain starts in a valid way.
    let mut last_item = match actions.next() {
        Some(item) => {
            match persisted_chain_head {
                Some((prev_hash, prev_seq)) => {
                    check_prev_action_chain(prev_hash, *prev_seq, item)?;
                }
                None => {
                    // If there's no persisted chain head, then the first action
                    // must have no parent.
                    if item.prev_hash().is_some() {
                        return Err((PrevActionErrorKind::InvalidRoot, item).into());
                    }
                }
            }
            (item.get_hash(), item.seq())
        }
        None => return Ok(()),
    };

    for item in actions {
        // Check each item of the chain is valid.
        check_prev_action_chain(last_item.0, last_item.1, item)?;
        last_item = (item.get_hash(), item.seq());
    }
    Ok(())
}

// Check the action is valid for the previous action.
fn check_prev_action_chain<A: ChainItem>(
    prev_action_hash: &A::Hash,
    prev_action_seq: u32,
    action: &A,
) -> Result<(), PrevActionError> {
    // The root cannot appear later in the chain
    if action.prev_hash().is_none() {
        Err((PrevActionErrorKind::MissingPrev, action).into())
    } else if action.prev_hash().map_or(true, |p| p != prev_action_hash) {
        // Check the prev hash matches.
        Err((PrevActionErrorKind::HashMismatch(action.seq()), action).into())
    } else if action
        .seq()
        .checked_sub(1)
        .map_or(true, |s| prev_action_seq != s)
    {
        // Check the prev seq is one less.
        Err((
            PrevActionErrorKind::InvalidSeq(action.seq(), prev_action_seq),
            action,
        )
            .into())
    } else {
        Ok(())
    }
}

/// Alias
pub type PrevActionResult<T> = Result<T, PrevActionError>;

/// Context information for an invalid action to make it easier to trace in errors.
#[derive(Error, Debug, Display, PartialEq, Eq)]
#[display(
    fmt = "{} - with context seq={}, action_hash={:?}, action=[{}]",
    source,
    seq,
    action_hash,
    action_display
)]
#[allow(missing_docs)]
pub struct PrevActionError {
    #[source]
    pub source: PrevActionErrorKind,
    pub seq: u32,
    pub action_hash: ActionHash,
    pub action_display: String,
}

impl<A: ChainItem> From<(PrevActionErrorKind, &A)> for PrevActionError {
    fn from((inner, action): (PrevActionErrorKind, &A)) -> Self {
        PrevActionError {
            source: inner,
            seq: action.seq(),
            action_hash: action.get_hash().clone().into(),
            action_display: action.to_display(),
        }
    }
}

impl From<(PrevActionErrorKind, Action)> for PrevActionError {
    fn from((inner, action): (PrevActionErrorKind, Action)) -> Self {
        PrevActionError {
            source: inner,
            seq: action.action_seq(),
            action_hash: action.to_hash(),
            action_display: format!("{}", action),
        }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum PrevActionErrorKind {
    #[error("The previous action hash specified in an action doesn't match the actual previous action. Seq: {0}")]
    HashMismatch(u32),
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Root of source chain must have a timestamp greater than the Dna's origin_time")]
    InvalidRootOriginTime,
    #[error("No more actions are allowed after a chain close")]
    ActionAfterChainClose,
    #[error("Previous action sequence number {1} != ({0} - 1)")]
    InvalidSeq(u32, u32),
    #[error("Action is not the first, so needs previous action")]
    MissingPrev,
    #[error("The previous action's timestamp is not before the current action's timestamp: {0:?} >= {1:?}")]
    Timestamp(Timestamp, Timestamp),
    #[error("The previous action's author does not match the current action's author: {0} != {1}")]
    Author(AgentPubKey, AgentPubKey),
    #[error("It is invalid for these two actions to be paired with each other. context: {0}, actions: {1:?}")]
    InvalidSuccessor(String, Box<(Action, Action)>),
}
