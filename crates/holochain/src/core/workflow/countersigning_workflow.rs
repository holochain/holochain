use std::collections::HashMap;

use holo_hash::EntryHash;
use holo_hash::{AgentPubKey, DhtOpHash, HeaderHash};
use holochain_p2p::HolochainP2pCellT;
use holochain_state::prelude::SourceChain;
use holochain_types::Timestamp;
use holochain_types::{dht_op::DhtOp, env::EnvWrite};
use holochain_zome_types::{Entry, SignedHeader};
use kitsune_p2p_types::tx2::tx2_utils::Share;

use crate::core::queue_consumer::{TriggerSender, WorkComplete};

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
    /// Map of header hash for a each signers header to the
    /// [`DhtOp`] and other required headers for this session to be
    /// considered complete.
    map: HashMap<HeaderHash, (DhtOpHash, DhtOp, Vec<HeaderHash>)>,
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
    mut trigger: TriggerSender,
) -> WorkflowResult<()> {
    let mut should_trigger = false;

    // For each op check it's the right type and extract the
    // entry hash, required headers and expires time.
    for (hash, op) in ops {
        // Must be a store entry op.
        if let DhtOp::StoreEntry(_, _, entry) = &op {
            // Must have a counter sign entry type.
            if let Entry::CounterSign(session_data, _) = entry.as_ref() {
                // Get the required headers for this session.
                let header_set = session_data.build_header_set()?;

                // Get the expires time for this session.
                let expires = *session_data
                    .preflight_request()
                    .session_times_ref()
                    .as_end_ref();

                // Get the entry hash from a header.
                // If the headers have different entry hashes they will fail validation.
                if let Some(entry_hash) = header_set.first().and_then(|h| h.entry_hash().cloned()) {
                    // Hash the required headers.
                    let required_headers: Vec<_> = header_set
                        .into_iter()
                        .map(|h| HeaderHash::with_data_sync(&h))
                        .collect();

                    // Check if already timed out.
                    if holochain_types::timestamp::now() < expires {
                        // Put this op in the pending map.
                        workspace.put(entry_hash, hash, op, required_headers, expires);
                        // We have new ops so we should trigger the workflow.
                        should_trigger = true;
                    }
                }
            }
        }
    }

    // Trigger the workflow if we have new ops.
    if should_trigger {
        trigger.trigger();
    }
    Ok(())
}

/// Countersigning workflow that checks for complete sessions and
/// pushes the complete ops to validation then messages the signers.
pub(crate) async fn countersigning_workflow(
    env: &EnvWrite,
    workspace: &CountersigningWorkspace,
    network: &(dyn HolochainP2pCellT + Send + Sync),
    sys_validation_trigger: &TriggerSender,
) -> WorkflowResult<WorkComplete> {
    // Get any complete sessions.
    let complete_sessions = workspace.get_complete_sessions();
    let mut notify_agents = Vec::with_capacity(complete_sessions.len());

    // For each complete session send the ops to validation.
    for (agents, ops, headers) in complete_sessions {
        incoming_dht_ops_workflow(&env, sys_validation_trigger.clone(), ops, false).await?;
        notify_agents.push((agents, headers));
    }

    // For each complete session notify the agents of success.
    for (agents, headers) in notify_agents {
        if let Err(e) = network
            .countersigning_authority_response(agents, headers)
            .await
        {
            // This could likely fail if a signer is offline so it's not really an error.
            tracing::info!(
                "Failed to notify agents: counter signed headers because of {:?}",
                e
            );
        }
    }
    Ok(WorkComplete::Complete)
}

pub(crate) async fn countersigning_success(
    vault: EnvWrite,
    author: AgentPubKey,
    signed_headers: Vec<SignedHeader>,
) -> WorkflowResult<()> {
    // let (persisted_head, persisted_seq) = vault
    //     .async_reader({
    //         let author = author.clone();
    //         move |txn| {
    //             // TODO: Check if chain is locked
    //             // is_chain_locked()
    //             let chain_locked = false;
    //             // chain_head_db(&txn, author)
    //             todo!()
    //         }
    //     })
    //     .await?;
    let source_chain = SourceChain::new(vault, author).await?;
    todo!()
}

type AgentsToNotify = Vec<AgentPubKey>;
type Ops = Vec<(DhtOpHash, DhtOp)>;
type SignedHeaders = Vec<SignedHeader>;

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
        required_headers: Vec<HeaderHash>,
        expires: Timestamp,
    ) {
        // hash the header of this ops.
        let header_hash = HeaderHash::with_data_sync(&op.header());
        self.inner
            .share_mut(|i, _| {
                // Get the session at this entry or create an empty one.
                let session = i.pending.entry(entry_hash).or_default();

                // Insert the op into the session.
                session
                    .map
                    .insert(header_hash, (op_hash, op, required_headers));

                // Set the expires time.
                session.expires = Some(expires);
                Ok(())
            })
            // We don't close this share so we can ignore this error.
            .ok();
    }

    fn get_complete_sessions(&self) -> Vec<(AgentsToNotify, Ops, SignedHeaders)> {
        let now = holochain_types::timestamp::now();
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
                        // If all session required headers are contained in the map
                        // then the session is complete.
                        if session.map.values().all(|(_, _, required_hashes)| {
                            required_hashes
                                .iter()
                                .all(|hash| session.map.contains_key(&hash))
                        }) {
                            Some(entry_hash.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut ret = Vec::with_capacity(complete.len());

                // For each complete session remove from the pending map
                // and fold into the signed headers to send to the agents
                // and the ops to validate.
                for hash in complete {
                    if let Some(session) = i.pending.remove(&hash) {
                        let map = session.map;
                        let r = map.into_iter().fold(
                            (Vec::new(), Vec::new(), Vec::new()),
                            |(mut agents, mut ops, mut headers), (_, (op_hash, op, _))| {
                                let header = op.header();
                                let signature = op.signature().clone();
                                // Agents to notify.
                                agents.push(header.author().clone());
                                // Signed headers to notify them with.
                                headers.push(SignedHeader(header, signature));
                                // Ops to validate.
                                ops.push((op_hash, op));
                                (agents, ops, headers)
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
    use holochain_types::timestamp;

    use super::*;

    #[test]
    /// Test that a session of 5 headers is complete when
    /// the expiry time is in the future and all required headers
    /// are present.
    fn gets_complete_sessions() {
        let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
        let workspace = CountersigningWorkspace::new();

        // - Create the ops.
        let data = |u: &mut arbitrary::Unstructured| {
            let op_hash = DhtOpHash::arbitrary(u).unwrap();
            let op = DhtOp::arbitrary(u).unwrap();
            let header = op.header();
            (op_hash, op, header)
        };
        let entry_hash = EntryHash::arbitrary(&mut u).unwrap();
        let mut op_hashes = Vec::new();
        let mut ops = Vec::new();
        let mut required_headers = Vec::new();
        for _ in 0..5 {
            let (op_hash, op, header) = data(&mut u);
            let header_hash = HeaderHash::with_data_sync(&header);
            op_hashes.push(op_hash);
            ops.push(op);
            required_headers.push(header_hash);
        }

        // - Put the ops in the workspace with expiry set to one hour from now.
        for (op_h, op) in op_hashes.into_iter().zip(ops.into_iter()) {
            let mut expires = timestamp::now();
            expires.0 += 60 * 60 * 1000;
            workspace.put(
                entry_hash.clone(),
                op_h,
                op,
                required_headers.clone(),
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
        let header = op.header();
        let entry_hash = EntryHash::arbitrary(&mut u).unwrap();
        let header_hash = HeaderHash::with_data_sync(&header);
        let mut expires = timestamp::now();
        expires.0 -= 60 * 60;

        // - Add it to the workspace.
        workspace.put(entry_hash, op_hash, op, vec![header_hash], expires);
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
