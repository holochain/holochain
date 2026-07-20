use crate::signature::Signed;
use crate::timestamp::Timestamp;
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_integrity_types::prelude::{
    Action, ActionBase, ActionData, ActionHeader, CounterSigningAgents, CounterSigningError,
    CounterSigningSessionData, CreateData, UpdateData, WrongActionError,
};
use holochain_serialized_bytes::prelude::*;
use thiserror::Error;

/// An [`Action`] with its [`Signature`](holochain_integrity_types::signature::Signature) (no hash).
pub type SignedAction = Signed<Action>;

#[derive(Error, Debug)]
pub enum ActionError {
    #[error(
        "Tried to create an entry-creating action from an action that isn't a Create or Update"
    )]
    NotNewEntry,
    #[error(transparent)]
    WrongActionError(#[from] WrongActionError),
    #[error("{0}")]
    Rebase(String),
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize, Default)]
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
    #[default]
    Strict,
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
        if matches!(self.data, ActionData::Dna(_)) {
            return Err(ActionError::Rebase("Rebased a DNA Action".to_string()));
        }
        let new_seq = new_prev_seq + 1;
        let new_timestamp = self.header.timestamp.max(
            (new_prev_timestamp + std::time::Duration::from_nanos(1))
                .map_err(|e| ActionError::Rebase(e.to_string()))?,
        );
        // Every non-DNA variant shares the same header shape, so rebasing
        // reduces to a single update of the common header fields.
        self.header.timestamp = new_timestamp;
        self.header.action_seq = new_seq;
        self.header.prev_action = Some(new_prev_action);
        Ok(())
    }
}

/// Build an [`Action`] from its common header and per-variant data.
///
/// Every action variant — including the genesis [`ActionData::Dna`], whose
/// header carries `prev_action: None` — shares the same [`ActionHeader`]
/// shape, so building an action is just pairing the two up.
pub fn build_action(header: ActionHeader, data: ActionData) -> Action {
    Action { header, data }
}

/// Build the [`Action`] a single agent contributes to a countersigning
/// session.
///
/// The action carries no weight; the model has no rate-limiting field.
pub fn from_countersigning_data(
    entry_hash: EntryHash,
    session_data: &CounterSigningSessionData,
    author: AgentPubKey,
) -> Result<Action, CounterSigningError> {
    let agent_state = session_data.agent_state_for_agent(&author)?;
    let header = ActionHeader {
        author,
        timestamp: session_data.to_timestamp(),
        action_seq: agent_state.action_seq() + 1,
        prev_action: Some(agent_state.chain_top().clone()),
    };
    let data = match &session_data.preflight_request().action_base {
        ActionBase::Create(base) => ActionData::Create(CreateData {
            entry_type: base.entry_type.clone(),
            entry_hash,
        }),
        ActionBase::Update(base) => ActionData::Update(UpdateData {
            original_action_address: base.original_action_address.clone(),
            original_entry_address: base.original_entry_address.clone(),
            entry_type: base.entry_type.clone(),
            entry_hash,
        }),
    };
    Ok(build_action(header, data))
}

/// Map a countersigning session to the ordered set of [`Action`]s each
/// participating agent contributes.
///
/// A given session always maps to the same ordered set of actions or an error.
/// The actions are not signed, since the intent is to build every
/// participant's action without holding their private key.
pub fn build_action_set(
    session_data: &CounterSigningSessionData,
    entry_hash: EntryHash,
) -> Result<Vec<Action>, CounterSigningError> {
    let mut actions = vec![];
    let mut build_actions =
        |countersigning_agents: &CounterSigningAgents| -> Result<(), CounterSigningError> {
            for (agent, _role) in countersigning_agents.iter() {
                actions.push(from_countersigning_data(
                    entry_hash.clone(),
                    session_data,
                    agent.clone(),
                )?);
            }
            Ok(())
        };
    build_actions(&session_data.preflight_request().signing_agents)?;
    build_actions(&session_data.preflight_request().optional_signing_agents)?;
    Ok(actions)
}
