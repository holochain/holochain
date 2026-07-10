//! Redesigned DHT state-model types (transitional — see `docs/design/state_model.md`).
//!
//! Re-exports the integrity-layer v2 types and adds the zome-layer aliases
//! [`SignedAction`] (data + signature) and [`SignedActionHashed`]
//! (content-addressed + signed). Also exposes the `op_type` INTEGER mapping
//! used by the DHT schema.

pub use holochain_integrity_types::dht_v2::*;

use crate::countersigning::{ActionBase, CounterSigningError, CounterSigningSessionData};
use crate::op::ChainOpType;
use crate::signature::Signed;
use holo_hash::{AgentPubKey, EntryHash};
use holochain_integrity_types::record::SignedHashed;

/// Build a v2 [`Action`] from its common header and per-variant data.
///
/// Every action variant — including the genesis [`ActionData::Dna`], whose
/// header carries `prev_action: None` — shares the same [`ActionHeader`]
/// shape, so building an action is just pairing the two up.
pub fn build_action(header: ActionHeader, data: ActionData) -> Action {
    Action { header, data }
}

/// Build the v2 [`Action`] a single agent contributes to a countersigning
/// session.
///
/// Mirrors the legacy `Action::from_countersigning_data`
/// (`holochain_integrity_types::countersigning`), but builds the v2
/// [`Action`] directly and carries no weight — the v2 model has no
/// rate-limiting field.
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

/// Map a countersigning session to the ordered set of v2 [`Action`]s each
/// participating agent contributes.
///
/// Mirrors the legacy `CounterSigningSessionData::build_action_set`
/// (`holochain_integrity_types::countersigning`), but produces v2
/// [`Action`]s (no weight). A given session always maps to the same ordered
/// set of actions or an error. The actions are not signed, since the intent
/// is to build every participant's action without holding their private key.
pub fn build_action_set(
    session_data: &CounterSigningSessionData,
    entry_hash: EntryHash,
) -> Result<Vec<Action>, CounterSigningError> {
    let mut actions = vec![];
    let mut build_actions =
        |countersigning_agents: &crate::countersigning::CounterSigningAgents| -> Result<(), CounterSigningError> {
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

/// A v2 [`Action`] with its [`crate::signature::Signature`] (no hash).
pub type SignedAction = Signed<Action>;

/// A v2 [`Action`] that is both hashed and signed.
pub type SignedActionHashed = SignedHashed<Action>;

/// A `Warrant` with its signature. Re-uses the existing `Warrant` type
/// from `holochain_zome_types::warrant` — unchanged by the v2 redesign.
pub use crate::warrant::SignedWarrant;

/// Maps [`ChainOpType`] onto the schema `op_type` INTEGER column (`1..=9`).
/// `0` is reserved and never written.
///
/// Variant ordering is pinned to `docs/design/state_model.md`:
///
/// | `op_type` | [`ChainOpType`] variant         | Semantic name  | Authority       |
/// |-----------|---------------------------------|----------------|-----------------|
/// | 1         | `StoreRecord`                   | CreateRecord   | action          |
/// | 2         | `StoreEntry`                    | CreateEntry    | entry           |
/// | 3         | `RegisterAgentActivity`         | AgentActivity  | agent           |
/// | 4         | `RegisterUpdatedContent`        | UpdateEntry    | entry           |
/// | 5         | `RegisterUpdatedRecord`         | UpdateRecord   | action          |
/// | 6         | `RegisterDeletedEntryAction`    | DeleteEntry    | entry           |
/// | 7         | `RegisterDeletedBy`             | DeleteRecord   | action          |
/// | 8         | `RegisterAddLink`               | CreateLink     | link base       |
/// | 9         | `RegisterRemoveLink`            | DeleteLink     | link base       |
impl From<ChainOpType> for i64 {
    fn from(t: ChainOpType) -> Self {
        match t {
            ChainOpType::StoreRecord => 1,
            ChainOpType::StoreEntry => 2,
            ChainOpType::RegisterAgentActivity => 3,
            ChainOpType::RegisterUpdatedContent => 4,
            ChainOpType::RegisterUpdatedRecord => 5,
            ChainOpType::RegisterDeletedEntryAction => 6,
            ChainOpType::RegisterDeletedBy => 7,
            ChainOpType::RegisterAddLink => 8,
            ChainOpType::RegisterRemoveLink => 9,
        }
    }
}

/// Inverse of [`From<ChainOpType> for i64`]. Returns `Err(v)` for `0` and any
/// value outside `1..=9`.
impl TryFrom<i64> for ChainOpType {
    type Error = i64;

    fn try_from(n: i64) -> Result<Self, Self::Error> {
        Ok(match n {
            1 => ChainOpType::StoreRecord,
            2 => ChainOpType::StoreEntry,
            3 => ChainOpType::RegisterAgentActivity,
            4 => ChainOpType::RegisterUpdatedContent,
            5 => ChainOpType::RegisterUpdatedRecord,
            6 => ChainOpType::RegisterDeletedEntryAction,
            7 => ChainOpType::RegisterDeletedBy,
            8 => ChainOpType::RegisterAddLink,
            9 => ChainOpType::RegisterRemoveLink,
            other => return Err(other),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_op_type_i64_roundtrip() {
        // Pinned forward-direction mapping. If a future change reorders
        // variants (e.g. a 6/7 swap) this will fail compilation or assertion.
        let expected = [
            (ChainOpType::StoreRecord, 1_i64),
            (ChainOpType::StoreEntry, 2),
            (ChainOpType::RegisterAgentActivity, 3),
            (ChainOpType::RegisterUpdatedContent, 4),
            (ChainOpType::RegisterUpdatedRecord, 5),
            (ChainOpType::RegisterDeletedEntryAction, 6),
            (ChainOpType::RegisterDeletedBy, 7),
            (ChainOpType::RegisterAddLink, 8),
            (ChainOpType::RegisterRemoveLink, 9),
        ];
        for (variant, n) in expected {
            assert_eq!(i64::from(variant), n);
            assert_eq!(ChainOpType::try_from(n).unwrap(), variant);
        }
        assert!(ChainOpType::try_from(0).is_err());
        assert!(ChainOpType::try_from(10).is_err());
    }
}
