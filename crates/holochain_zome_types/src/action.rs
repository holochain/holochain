use crate::timestamp::Timestamp;
pub use builder::ActionBuilder;
pub use builder::ActionBuilderCommon;
use conversions::WrongActionError;
use holo_hash::ActionHash;
use holochain_serialized_bytes::prelude::*;
use thiserror::Error;

pub use holochain_integrity_types::action::*;

#[cfg(any(test, feature = "test_utils"))]
pub use facts::*;

pub mod builder;
#[cfg(any(test, feature = "test_utils"))]
pub mod facts;

#[derive(Error, Debug)]
pub enum ActionError {
    #[error("Tried to create a NewEntryAction with a type that isn't a Create or Update")]
    NotNewEntry,
    #[error(transparent)]
    WrongActionError(#[from] WrongActionError),
    #[error("{0}")]
    Rebase(String),
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChainTopOrdering {
    /// Relaxed chain top ordering REWRITES ACTIONS INLINE during a flush of
    /// the source chain to sit on top of the current chain top. The "as at"
    /// of the zome call initial state is completely ignored.
    /// This may be significantly more efficient if you are CERTAIN that none
    /// of your zome or validation logic is order dependent. Examples include
    /// simple chat messages or tweets. Note however that even chat messages
    /// and tweets may have subtle order dependencies, such as if a cap grant
    /// was written or revoked that would have invalidated the zome call that
    /// wrote data after the revocation, etc.
    /// The efficiency of relaxed ordering comes from simply rehashing and
    /// signing actions on the new chain top during flush, avoiding the
    /// overhead of the client, websockets, zome call instance, wasm execution,
    /// validation, etc. that would result from handling a `HeadMoved` error
    /// via an external driver.
    Relaxed,
    /// The default `Strict` ordering is the default for a very good reason.
    /// Writes normally compare the chain head from the start of a zome call
    /// against the time a write transaction is flushed from the source chain.
    /// This is REQUIRED for data integrity if any zome or validation logic
    /// depends on the ordering of data in a chain.
    /// This order dependence could be obvious such as an explicit reference or
    /// dependency. It could be very subtle such as checking for the existence
    /// or absence of some data.
    /// If you are unsure whether your data is order dependent you should err
    /// on the side of caution and handle `HeadMoved` errors on the client of
    /// the zome call and restart the zome call from the start.
    Strict,
}

impl Default for ChainTopOrdering {
    fn default() -> Self {
        Self::Strict
    }
}

pub trait ActionExt {
    fn rebase_on(
        &mut self,
        new_prev_action: ActionHash,
        new_prev_seq: u32,
        new_prev_timestamp: Timestamp,
    ) -> Result<(), ActionError>;
}

impl ActionExt for Action {
    fn rebase_on(
        &mut self,
        new_prev_action: ActionHash,
        new_prev_seq: u32,
        new_prev_timestamp: Timestamp,
    ) -> Result<(), ActionError> {
        let new_seq = new_prev_seq + 1;
        let new_timestamp = self.timestamp().max(
            (new_prev_timestamp + std::time::Duration::from_nanos(1))
                .map_err(|e| ActionError::Rebase(e.to_string()))?,
        );
        match self {
            Self::Dna(_) => return Err(ActionError::Rebase("Rebased a DNA Action".to_string())),
            Self::AgentValidationPkg(AgentValidationPkg {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::CreateLink(CreateLink {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::DeleteLink(DeleteLink {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::Delete(Delete {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::CloseChain(CloseChain {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::OpenChain(OpenChain {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::Create(Create {
                timestamp,
                action_seq,
                prev_action,
                ..
            })
            | Self::Update(Update {
                timestamp,
                action_seq,
                prev_action,
                ..
            }) => {
                *timestamp = new_timestamp;
                *action_seq = new_seq;
                *prev_action = new_prev_action;
            }
        };
        Ok(())
    }
}
