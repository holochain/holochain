use std::collections::HashMap;
use std::sync::Arc;

use holo_hash::{ActionHash, AgentPubKey, DhtOpHash};
use holo_hash::{AnyDhtHash, EntryHash};
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::integrate::authored_ops_to_dht_db;
use holochain_state::mutations;
use holochain_state::prelude::{
    current_countersigning_session, SourceChainResult, StateMutationResult, Store,
};
use holochain_types::dht_op::DhtOp;
use holochain_types::signal::{Signal, SystemSignal};
use holochain_zome_types::Timestamp;
use holochain_zome_types::{Entry, SignedAction, ZomeCallResponse};
use kitsune_p2p_types::tx2::tx2_utils::Share;
use rusqlite::{named_params, Transaction};

use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::space::Space;
use crate::core::queue_consumer::{QueueTriggers, TriggerSender, WorkComplete};
use crate::core::ribosome::weigh_placeholder;

use super::{error::WorkflowResult, incoming_dht_ops_workflow::incoming_dht_ops_workflow};

#[derive(Clone)]
/// A cheaply clonable, thread safe and in-memory store for
/// active countersigning sessions.
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
}

#[derive(Default)]
/// Pending countersigning sessions.
pub struct CountersigningWorkspaceInner {
    pending: HashMap<EntryHash, Session>,
}

#[derive(Default)]
struct Session {
    /// Map of action hash for a each signers action to the
    /// [`DhtOp`] and other required actions for this session to be
    /// considered complete.
    map: HashMap<ActionHash, (DhtOpHash, DhtOp, Vec<ActionHash>)>,
    /// When this session expires.
    /// If this is none the session is empty.
    expires: Option<Timestamp>,
}

/// New incoming DhtOps for a countersigning session.
// TODO: PERF: This takes a lock on the workspace which could
// block other incoming DhtOps if there are many active sessions.
// We could create an incoming buffer if this actually becomes an issue.
pub(crate) fn incoming_countersigning(
    ops: Vec<(DhtOpHash, DhtOp)>,
    workspace: &CountersigningWorkspace,
    trigger: TriggerSender,
) -> WorkflowResult<()> {
    let mut should_trigger = false;

    // For each op check it's the right type and extract the
    // entry hash, required actions and expires time.
    for (hash, op) in ops {
        // Must be a store entry op.
        if let DhtOp::StoreEntry(_, _, entry) = &op {
            // Must have a counter sign entry type.
            if let Entry::CounterSign(session_data, _) = entry.as_ref() {
                let entry_hash = EntryHash::with_data_sync(&**entry);
                // Get the required actions for this session.
                let weight = weigh_placeholder();
                let action_set = session_data.build_action_set(entry_hash, weight)?;

                // Get the expires time for this session.
                let expires = *session_data.preflight_request().session_times().end();

                // Get the entry hash from a action.
                // If the actions have different entry hashes they will fail validation.
                if let Some(entry_hash) = action_set.first().and_then(|h| h.entry_hash().cloned()) {
                    // Hash the required actions.
                    let required_actions: Vec<_> = action_set
                        .into_iter()
                        .map(|h| ActionHash::with_data_sync(&h))
                        .collect();

                    // Check if already timed out.
                    if holochain_zome_types::Timestamp::now() < expires {
                        // Put this op in the pending map.
                        workspace.put(entry_hash, hash, op, required_actions, expires);
                        // We have new ops so we should trigger the workflow.
                        should_trigger = true;
                    }
                }
            }
        }
    }

    // Trigger the workflow if we have new ops.
    if should_trigger {
        trigger.trigger(&"incoming_countersigning");
    }
    Ok(())
}

/// Countersigning workflow that checks for complete sessions and
/// pushes the complete ops to validation then messages the signers.
pub(crate) async fn countersigning_workflow(
    space: &Space,
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    sys_validation_trigger: &TriggerSender,
) -> WorkflowResult<WorkComplete> {
    // Get any complete sessions.
    let complete_sessions = space.countersigning_workspace.get_complete_sessions();
    let mut notify_agents = Vec::with_capacity(complete_sessions.len());

    // For each complete session send the ops to validation.
    for (agents, ops, actions) in complete_sessions {
        incoming_dht_ops_workflow(space, sys_validation_trigger.clone(), ops, false).await?;
        notify_agents.push((agents, actions));
    }

    // For each complete session notify the agents of success.
    for (agents, actions) in notify_agents {
        if let Err(e) = network
            .countersigning_authority_response(agents, actions)
            .await
        {
            // This could likely fail if a signer is offline so it's not really an error.
            tracing::info!(
                "Failed to notify agents: counter signed actions because of {:?}",
                e
            );
        }
    }
    Ok(WorkComplete::Complete)
}

/// An incoming countersigning session success.
pub(crate) async fn countersigning_success(
    space: Space,
    network: &HolochainP2pDna,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    trigger: QueueTriggers,
    mut signal: SignalBroadcaster,
) -> WorkflowResult<()> {
    let authored_db = space.authored_db;
    let dht_db = space.dht_db;
    let dht_db_cache = space.dht_query_cache;
    let QueueTriggers {
        publish_dht_ops: publish_trigger,
        integrate_dht_ops: integration_trigger,
        ..
    } = trigger;
    // Using iterators is fine in this function as there can only be a maximum of 8 actions.
    let (this_cells_action_hash, entry_hash) = match signed_actions
        .iter()
        .find(|h| *h.0.author() == author)
        .and_then(|sh| {
            sh.0.entry_hash()
                .cloned()
                .map(|eh| (ActionHash::with_data_sync(&sh.0), eh))
        }) {
        Some(h) => h,
        None => return Ok(()),
    };

    // Do a quick check to see if this entry hash matches
    // the current locked session so we don't check signatures
    // unless there is an active session.
    let reader_closure = {
        let entry_hash = entry_hash.clone();
        let this_cells_action_hash = this_cells_action_hash.clone();
        let author = author.clone();
        move |txn: Transaction| {
            if holochain_state::chain_lock::is_chain_locked(&txn, &[], &author)? {
                let transaction: holochain_state::prelude::Txn = (&txn).into();
                if transaction.contains_entry(&entry_hash)? {
                    // If this is a countersigning session we can grab all the ops
                    // for this cells action so we can check if we need to self publish them.
                    let r: Result<_, _> = txn
                        .prepare(
                            "SELECT basis_hash, hash FROM DhtOp WHERE action_hash = :action_hash",
                        )?
                        .query_map(
                            named_params! {
                                ":action_hash": this_cells_action_hash
                            },
                            |row| {
                                let hash: DhtOpHash = row.get("hash")?;
                                let basis: AnyDhtHash = row.get("basis_hash")?;
                                Ok((hash, basis))
                            },
                        )?
                        .collect();
                    return Ok(r?);
                }
            }
            StateMutationResult::Ok(Vec::with_capacity(0))
        }
    };
    let this_cell_actions_op_basis_hashes: Vec<(DhtOpHash, AnyDhtHash)> =
        authored_db.async_reader(reader_closure).await?;

    // If there is no active session then we can short circuit.
    if this_cell_actions_op_basis_hashes.is_empty() {
        return Ok(());
    }

    // Verify signatures of actions.
    for SignedAction(action, signature) in &signed_actions {
        if !action.author().verify_signature(signature, action).await {
            return Ok(());
        }
    }

    // Hash actions.
    let incoming_actions: Vec<_> = signed_actions
        .iter()
        .map(|SignedAction(h, _)| ActionHash::with_data_sync(h))
        .collect();

    let result = authored_db
        .async_commit({
            let author = author.clone();
            let entry_hash = entry_hash.clone();
            move |txn| {
            if let Some((cs_entry_hash, cs)) = current_countersigning_session(txn, Arc::new(author.clone()))? {
                // Check we have the right session.
                if cs_entry_hash == entry_hash {
                    let weight = weigh_placeholder();
                    let stored_actions = cs.build_action_set(entry_hash, weight)?;
                    if stored_actions.len() == incoming_actions.len() {
                        // Check all stored action hashes match an incoming action hash.
                        if stored_actions.iter().all(|h| {
                            let h = ActionHash::with_data_sync(h);
                            incoming_actions.iter().any(|i| *i == h)
                        }) {
                            // All checks have passed so unlock the chain.
                            mutations::unlock_chain(txn, &author)?;
                            // Update ops to publish.
                            txn.execute("UPDATE DhtOp SET withhold_publish = NULL WHERE action_hash = :action_hash",
                            named_params! {
                                ":action_hash": this_cells_action_hash,
                                }
                            ).map_err(holochain_state::prelude::StateMutationError::from)?;
                            return Ok(true);
                        }
                    }
                }
            }
            SourceChainResult::Ok(false)
        }})
        .await?;

    if result {
        authored_ops_to_dht_db(
            network,
            this_cell_actions_op_basis_hashes,
            &(authored_db.into()),
            &dht_db,
            &dht_db_cache,
        )
        .await?;
        integration_trigger.trigger(&"countersigning_success");
        // Publish other signers agent activity ops to their agent activity authorities.
        for SignedAction(action, signature) in signed_actions {
            if *action.author() == author {
                continue;
            }
            let op = DhtOp::RegisterAgentActivity(signature, action);
            let basis = op.dht_basis();
            let ops = vec![op];
            if let Err(e) = network.publish(false, false, basis, ops, None).await {
                tracing::error!(
                    "Failed to publish to other countersigners agent authorities because of: {:?}",
                    e
                );
            }
        }
        // Signal to the UI.
        signal.send(Signal::System(SystemSignal::SuccessfulCountersigning(
            entry_hash,
        )))?;

        publish_trigger.trigger(&"publish countersigning_success");
    }
    Ok(())
}

/// Publish to entry authorities so they can gather all the signed
/// actions for this session and respond with a session complete.
pub async fn countersigning_publish(
    network: &HolochainP2pDna,
    op: DhtOp,
) -> Result<(), ZomeCallResponse> {
    let basis = op.dht_basis();
    let ops = vec![op];
    if let Err(e) = network.publish(false, true, basis, ops, None).await {
        tracing::error!(
            "Failed to publish to entry authorities for countersigning session because of: {:?}",
            e
        );
        return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
    }
    Ok(())
}

type AgentsToNotify = Vec<AgentPubKey>;
type Ops = Vec<(DhtOpHash, DhtOp)>;
type SignedActions = Vec<SignedAction>;

impl CountersigningWorkspace {
    /// Create a new empty countersigning workspace.
    pub fn new() -> CountersigningWorkspace {
        Self {
            inner: Share::new(Default::default()),
        }
    }

    /// Put a single signers store entry op in the workspace.
    fn put(
        &self,
        entry_hash: EntryHash,
        op_hash: DhtOpHash,
        op: DhtOp,
        required_actions: Vec<ActionHash>,
        expires: Timestamp,
    ) {
        // hash the action of this ops.
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
            // We don't close this share so we can ignore this error.
            .ok();
    }

    fn get_complete_sessions(&self) -> Vec<(AgentsToNotify, Ops, SignedActions)> {
        let now = holochain_zome_types::Timestamp::now();
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
                                actions.push(SignedAction(action, signature));
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

impl Default for CountersigningWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::Arbitrary;

    use super::*;

    #[test]
    /// Test that a session of 5 actions is complete when
    /// the expiry time is in the future and all required actions
    /// are present.
    fn gets_complete_sessions() {
        let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
        let workspace = CountersigningWorkspace::new();

        // - Create the ops.
        let data = |u: &mut arbitrary::Unstructured| {
            let op_hash = DhtOpHash::arbitrary(u).unwrap();
            let op = DhtOp::arbitrary(u).unwrap();
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

    #[test]
    /// Test that expired sessions are removed.
    fn expired_sessions_removed() {
        let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
        let workspace = CountersigningWorkspace::new();

        // - Create an op for a session that has expired in the past.
        let op_hash = DhtOpHash::arbitrary(&mut u).unwrap();
        let op = DhtOp::arbitrary(&mut u).unwrap();
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
