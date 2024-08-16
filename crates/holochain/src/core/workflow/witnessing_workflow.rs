//! Witnessing workflow that is the counterpart to the countersigning workflow.

use super::{error::WorkflowResult, incoming_dht_ops_workflow::incoming_dht_ops_workflow};
use crate::conductor::space::Space;
use crate::core::queue_consumer::{TriggerSender, WorkComplete};
use crate::core::ribosome::weigh_placeholder;
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash};
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use std::collections::HashMap;

/// A cheaply cloneable, thread-safe and in-memory store for
/// active countersigning sessions.
#[derive(Clone, Default)]
pub struct WitnessingWorkspace {
    inner: Share<WitnessingWorkspaceInner>,
}

/// Pending countersigning sessions.
#[derive(Default)]
pub struct WitnessingWorkspaceInner {
    pending: HashMap<EntryHash, Session>,
}

#[derive(Default)]
struct Session {
    /// Map of action hash for each signers action to the [`DhtOp`] and other required actions for
    /// this session to be considered complete.
    map: HashMap<ActionHash, (DhtOpHash, ChainOp, Vec<ActionHash>)>,
    /// When this session expires.
    ///
    /// If this is none the session is empty.
    expires: Option<Timestamp>,
}

/// Witnessing workflow that is the counterpart to the countersigning workflow.
///
/// This workflow is run by witnesses to countersigning sessions who are responsible for gathering
/// signatures during sessions. The workflow checks for complete sessions and pushes the complete
/// ops to validation then messages the session participants with the complete set of signatures
/// for the session.
pub(crate) async fn witnessing_workflow(
    space: Space,
    network: impl HolochainP2pDnaT,
    sys_validation_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    // Get any complete sessions.
    let complete_sessions = space.witnessing_workspace.get_complete_sessions();
    let mut notify_agents = Vec::with_capacity(complete_sessions.len());

    // For each complete session send the ops to validation.
    for (agents, ops, actions) in complete_sessions {
        let non_enzymatic_ops: Vec<_> = ops
            .into_iter()
            .filter(|(_hash, dht_op)| dht_op.enzymatic_countersigning_enzyme().is_none())
            .collect();
        if !non_enzymatic_ops.is_empty() {
            incoming_dht_ops_workflow(
                space.clone(),
                sys_validation_trigger.clone(),
                non_enzymatic_ops
                    .into_iter()
                    .map(|(_h, o)| o.into())
                    .collect(),
                false,
            )
            .await?;
        }
        notify_agents.push((agents, actions));
    }

    // For each complete session notify the agents of success.
    for (agents, actions) in notify_agents {
        if let Err(e) = network
            .countersigning_session_negotiation(
                agents,
                CountersigningSessionNegotiationMessage::AuthorityResponse(actions),
            )
            .await
        {
            // This could likely fail if a signer is offline, so it's not an error.
            tracing::warn!(
                "Failed to notify agents: counter signed actions because of {:?}",
                e
            );
        }
    }
    Ok(WorkComplete::Complete)
}

/// Receive incoming DhtOps for a countersigning session.
///
/// These ops are produced by participants in a countersigning session and sent to us to be checked.
/// This function will store the ops in the workspace and trigger the workflow.
pub(crate) fn receive_incoming_countersigning_ops(
    ops: Vec<(DhtOpHash, ChainOp)>,
    workspace: &WitnessingWorkspace,
    witnessing_workflow_trigger: TriggerSender,
) -> WorkflowResult<()> {
    let mut should_trigger = false;

    // For each op check it's the right type and extract the
    // entry hash, required actions and expires time.
    for (hash, op) in ops {
        // Must be a store entry op.
        if let ChainOp::StoreEntry(_, _, entry) = &op {
            // Must be a CounterSign entry type.
            if let Entry::CounterSign(session_data, _) = entry {
                let entry_hash = EntryHash::with_data_sync(entry);
                // Get the required actions for this session.
                let weight = weigh_placeholder();
                let action_set = session_data.build_action_set(entry_hash, weight)?;

                // Get the expires time for this session.
                let expires = *session_data.preflight_request().session_times.end();

                // Get the entry hash from an action.
                // If the actions have different entry hashes they will fail validation.
                if let Some(entry_hash) = action_set.first().and_then(|a| a.entry_hash().cloned()) {
                    // Hash the required actions.
                    let required_actions: Vec<_> = action_set
                        .into_iter()
                        .map(|a| ActionHash::with_data_sync(&a))
                        .collect();

                    // Only accept the op if the session is not expired.
                    if Timestamp::now() < expires {
                        // Put this op in the pending map.
                        workspace.put(entry_hash, hash, op, required_actions, expires);
                        // We have new ops, so we should trigger the workflow.
                        should_trigger = true;
                    }
                }
            } else {
                tracing::warn!(?op, "Incoming countersigning op is not a CounterSign entry");
            }
        } else {
            tracing::warn!(?op, "Incoming countersigning op is not a StoreEntry op");
        }
    }

    // Trigger the workflow if we have new ops.
    if should_trigger {
        witnessing_workflow_trigger.trigger(&"incoming_countersigning");
    }
    Ok(())
}

type AgentsToNotify = Vec<AgentPubKey>;
type Ops = Vec<(DhtOpHash, ChainOp)>;
type SignedActions = Vec<SignedAction>;

impl WitnessingWorkspace {
    /// Create a new empty countersigning workspace.
    pub fn new() -> WitnessingWorkspace {
        Self {
            inner: Share::new(Default::default()),
        }
    }

    /// Put a single signers store entry op in the workspace.
    fn put(
        &self,
        entry_hash: EntryHash,
        op_hash: DhtOpHash,
        op: ChainOp,
        required_actions: Vec<ActionHash>,
        expires: Timestamp,
    ) {
        // Hash the action of this op.
        let action_hash = ActionHash::with_data_sync(&op.action());
        self.inner
            .share_mut(|i, _| {
                // Get the session at this entry or create an empty one.
                let session = i.pending.entry(entry_hash).or_default();

                // Insert the op into the session.
                session
                    .map
                    .insert(action_hash, (op_hash, op, required_actions));

                // Set the expires time.
                session.expires = Some(expires);
                Ok(())
            })
            // We don't close this share, so we can ignore this error.
            .ok();
    }

    fn get_complete_sessions(&self) -> Vec<(AgentsToNotify, Ops, SignedActions)> {
        let now = Timestamp::now();
        self.inner
            .share_mut(|i, _| {
                // Remove any expired sessions.
                i.pending.retain(|_, session| {
                    session.expires.as_ref().map(|e| now < *e).unwrap_or(false)
                });

                // Get all complete session's entry hashes.
                let complete: Vec<_> = i
                    .pending
                    .iter()
                    .filter_map(|(entry_hash, session)| {
                        // If all session required actions are contained in the map
                        // then the session is complete.
                        if session.map.values().all(|(_, _, required_hashes)| {
                            required_hashes
                                .iter()
                                .all(|hash| session.map.contains_key(hash))
                        }) {
                            Some(entry_hash.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut ret = Vec::with_capacity(complete.len());

                // For each complete session remove from the pending map
                // and fold into the signed actions to send to the agents
                // and the ops to validate.
                for hash in complete {
                    if let Some(session) = i.pending.remove(&hash) {
                        let map = session.map;
                        let r = map.into_iter().fold(
                            (Vec::new(), Vec::new(), Vec::new()),
                            |(mut agents, mut ops, mut actions), (_, (op_hash, op, _))| {
                                let action = op.action();
                                let signature = op.signature().clone();
                                // Agents to notify.
                                agents.push(action.author().clone());
                                // Signed actions to notify them with.
                                actions.push(SignedAction::new(action, signature));
                                // Ops to validate.
                                ops.push((op_hash, op));
                                (agents, ops, actions)
                            },
                        );
                        ret.push(r);
                    }
                }
                Ok(ret)
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbitrary::Arbitrary;

    /// Test that a session of 5 actions is complete when the expiry time is in the future and all
    /// required actions are present.
    #[test]
    fn gets_complete_sessions() {
        let mut u = arbitrary::Unstructured::new(&NOISE);
        let workspace = WitnessingWorkspace::new();

        // - Create the ops.
        let data = |u: &mut arbitrary::Unstructured| {
            let op_hash = DhtOpHash::arbitrary(u).unwrap();
            let op = ChainOp::arbitrary(u).unwrap();
            let action = op.action();
            (op_hash, op, action)
        };
        let entry_hash = EntryHash::arbitrary(&mut u).unwrap();
        let mut op_hashes = Vec::new();
        let mut ops = Vec::new();
        let mut required_actions = Vec::new();
        for _ in 0..5 {
            let (op_hash, op, action) = data(&mut u);
            let action_hash = ActionHash::with_data_sync(&action);
            op_hashes.push(op_hash);
            ops.push(op);
            required_actions.push(action_hash);
        }

        // - Put the ops in the workspace with expiry set to one hour from now.
        for (op_h, op) in op_hashes.into_iter().zip(ops.into_iter()) {
            let expires = (Timestamp::now() + std::time::Duration::from_secs(60 * 60)).unwrap();
            workspace.put(
                entry_hash.clone(),
                op_h,
                op,
                required_actions.clone(),
                expires,
            );
        }

        // - Get all complete sessions.
        let r = workspace.get_complete_sessions();
        // - Expect we have one.
        assert_eq!(r.len(), 1);

        workspace
            .inner
            .share_mut(|i, _| {
                // - Check we have none pending.
                assert_eq!(i.pending.len(), 0);
                Ok(())
            })
            .unwrap();
    }

    /// Test that expired sessions are removed.
    #[test]
    fn expired_sessions_removed() {
        let mut u = arbitrary::Unstructured::new(&NOISE);
        let workspace = WitnessingWorkspace::new();

        // - Create an op for a session that has expired in the past.
        let op_hash = DhtOpHash::arbitrary(&mut u).unwrap();
        let op = ChainOp::arbitrary(&mut u).unwrap();
        let action = op.action();
        let entry_hash = EntryHash::arbitrary(&mut u).unwrap();
        let action_hash = ActionHash::with_data_sync(&action);
        let expires = (Timestamp::now() - std::time::Duration::from_secs(60 * 60)).unwrap();

        // - Add it to the workspace.
        workspace.put(entry_hash, op_hash, op, vec![action_hash], expires);
        let r = workspace.get_complete_sessions();

        // - Expect we have no complete sessions.
        assert_eq!(r.len(), 0);
        workspace
            .inner
            .share_mut(|i, _| {
                // - Check we have none pending.
                assert_eq!(i.pending.len(), 0);
                Ok(())
            })
            .unwrap();
    }
}
