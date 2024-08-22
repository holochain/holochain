//! Countersigning workflow to maintain countersigning session state.

#![allow(unused)]

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::{QueueTriggers, TriggerSender, WorkComplete};
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::WorkflowError;
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, OpBasis};
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::mutations;
use holochain_state::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::KitsuneError;
use rusqlite::{named_params, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

mod accept;

pub(crate) use accept::accept_countersigning_request;

/// Countersigning workspace to hold session state.
#[derive(Clone, Default)]
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    sessions: HashMap<AgentPubKey, SessionState>,
}

#[derive(Debug)]
enum SessionState {
    Accepted(PreflightRequest),
    /// Multiple responses in the outer vec, sets of responses in the inner vec
    SignaturesCollected(Vec<Vec<SignedAction>>),
    FailedValidation,
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub(crate) async fn countersigning_workflow(
    space: Space,
    network: impl HolochainP2pDnaT,
    cell_id: CellId,
    conductor: ConductorHandle,
    self_trigger: TriggerSender,
    sys_validation_trigger: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    tracing::debug!(
        "Starting countersigning workflow, with {} sessions",
        space
            .countersigning_workspace
            .inner
            .share_ref(|inner| Ok(inner.sessions.len()))
            .unwrap()
    );

    let signal_tx = conductor
        .get_signal_tx(&cell_id)
        .await
        .map_err(|e| WorkflowError::other(e))?;

    let timed_out_sessions = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    SessionState::Accepted(request) => {
                        if request.session_times.end < Timestamp::now() {
                            Some((author.clone(), request.clone()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    for (author, request) in timed_out_sessions {
        // TODO resolved timed out sessions
        tracing::warn!("Countersigning session timed out for agent: {:?}", author);
    }

    let completed_sessions = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    SessionState::SignaturesCollected(signatures) => {
                        Some((author.clone(), signatures.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    for (author, signatures) in completed_sessions {
        for signature_bundle in signatures {
            // Try to complete the session using this signature bundle.
            if let Ok(true) = inner_countersigning_session_complete(
                space.clone(),
                &network,
                author.clone(),
                signature_bundle.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
                signal_tx.clone(),
            )
            .await
            {
                // If the session completed successfully with this bundle then we can remove the
                // session from the workspace.
                space
                    .countersigning_workspace
                    .inner
                    .share_mut(|inner, _| {
                        inner.sessions.remove(&author);
                        Ok(())
                    })
                    .unwrap();
                break;
            }
        }
    }

    // At the end of the workflow, if we have any sessions still in progress, then schedule a
    // workflow run for the one that will finish soonest.
    let maybe_earliest_finish = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| Ok(inner.sessions.values().filter_map(|s| {
            match s {
                SessionState::Accepted(request) => Some(request.session_times.end),
                state => {
                    tracing::warn!("Countersigning session should be resolved but is still the workspace for agent: {:?}", state);
                    None
                }
            }
        }).min()))
        .unwrap();
    if let Some(earliest_finish) = maybe_earliest_finish {
        let delay = match (earliest_finish - Timestamp::now()).map(|d| d.to_std()) {
            Ok(Ok(d)) => d,
            _ => Duration::from_millis(100),
        };
        tracing::debug!("Countersigning workflow will run again in {:?}", delay);
        Ok(WorkComplete::Incomplete(Some(delay)))
    } else {
        Ok(WorkComplete::Complete)
    }
}

/// An incoming countersigning session success.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub(crate) async fn countersigning_success(
    space: Space,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    countersigning_trigger: TriggerSender,
) {
    let should_trigger = space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            match inner.sessions.entry(author.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    match entry.get_mut() {
                        SessionState::Accepted(_) => {
                            entry.insert(SessionState::SignaturesCollected(vec![signed_actions]));
                        }
                        SessionState::SignaturesCollected(sigs) => {
                            sigs.push(signed_actions);
                        }
                        SessionState::FailedValidation => {
                            // This is not expected, we shouldn't have published a signature if our session failed validation.
                            // Where did this remote get a full set of signatures from to send to us?
                            tracing::warn!("Countersigning session success received but the the session has failed validation locally");
                            return Ok(false);
                        }
                    }
                }
                std::collections::hash_map::Entry::Vacant(_) => {
                    // This will happen if the session has already been resolved and removed from
                    // internal state. Or if the conductor has restarted.
                    tracing::trace!("Countersigning session signatures received but is not in the workspace for agent: {:?}", author);
                    return Ok(false);
                }
            }

            Ok(true)
        })
        // Unwrap the error, because this share_mut callback doesn't return an error.
        .unwrap();

    if should_trigger {
        tracing::debug!("Received a signature bundle, triggering countersigning workflow");
        countersigning_trigger.trigger(&"countersigning_success");
    }
}

pub(crate) async fn inner_countersigning_session_complete(
    space: Space,
    network: &impl HolochainP2pDnaT,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
    signal: broadcast::Sender<Signal>,
) -> WorkflowResult<bool> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;
    let dht_db = space.dht_db;
    let dht_db_cache = space.dht_query_cache;

    // Using iterators is fine in this function as there can only be a maximum of 8 actions.
    let (this_cells_action_hash, entry_hash) = match signed_actions
        .iter()
        .find(|a| *a.author() == author)
        .and_then(|sa| {
            sa.entry_hash()
                .cloned()
                .map(|eh| (ActionHash::with_data_sync(sa), eh))
        }) {
        Some(a) => a,
        None => return Ok(false),
    };

    // Do a quick check to see if this entry hash matches the current locked session, so we don't
    // check signatures unless there is an active session.
    let reader_closure = {
        let entry_hash = entry_hash.clone();
        let this_cells_action_hash = this_cells_action_hash.clone();
        let author = author.clone();
        move |txn: Transaction| {
            // This chain lock isn't necessarily for the current session, we can't check that until later.
            if let Some((cs_entry_hash, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = holo_hash::encode::blake2b_256(
                    &holochain_serialized_bytes::encode(&session_data.preflight_request())?,
                );

                let chain_lock = holochain_state::chain_lock::get_chain_lock(&txn, &author)?;
                if let Some(chain_lock) = chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if cs_entry_hash != entry_hash || chain_lock.subject() != &lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    let transaction: holochain_state::prelude::Txn = (&txn).into();
                    if transaction.contains_entry(&entry_hash)? {
                        // If this is a countersigning session we can grab all the ops for this
                        // cells action, so we can check if we need to self-publish them.
                        let r = txn
                            .prepare(
                                "SELECT basis_hash, hash FROM DhtOp WHERE action_hash = :action_hash",
                            ).map_err(DatabaseError::from)?
                            .query_map(
                                named_params! {
                                ":action_hash": this_cells_action_hash
                            },
                                |row| {
                                    let hash: DhtOpHash = row.get("hash")?;
                                    let basis: OpBasis = row.get("basis_hash")?;
                                    Ok((hash, basis))
                                },
                            ).map_err(DatabaseError::from)?
                            .collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)?;
                        return Ok(Some((session_data, r)));
                    }
                }
            }
            SourceChainResult::Ok(None)
        }
    };

    let (session_data, this_cell_actions_op_basis_hashes) = match authored_db
        .read_async(reader_closure)
        .await?
    {
        Some((cs, r)) => (cs, r),
        None => {
            // If there is no active session then we can short circuit.
            tracing::warn!("Received a signature bundle for a session that exists in state but is missing from the database");
            return Ok(false);
        }
    };

    // Verify signatures of actions.
    let mut i_am_an_author = false;
    for sa in &signed_actions {
        if !sa
            .action()
            .author()
            .verify_signature(sa.signature(), sa.action())
            .await?
        {
            return Ok(false);
        }
        if sa.action().author() == &author {
            i_am_an_author = true;
        }
    }

    // Countersigning success is ultimately between authors to agree and publish.
    if !i_am_an_author {
        // We're effectively rejecting this signature bundle but communicate that this signature
        // bundle wasn't acceptable so that we can try another one.
        return Ok(false);
    }

    // Hash actions.
    let incoming_actions: Vec<_> = signed_actions
        .iter()
        .map(ActionHash::with_data_sync)
        .collect();

    let result = authored_db
        .write_async({
            let author = author.clone();
            let entry_hash = entry_hash.clone();
            move |txn| {
                let weight = weigh_placeholder();
                let stored_actions = session_data.build_action_set(entry_hash, weight)?;
                if stored_actions.len() == incoming_actions.len() {
                    // Check all stored action hashes match an incoming action hash.
                    if stored_actions.iter().all(|a| {
                        let a = ActionHash::with_data_sync(a);
                        incoming_actions.iter().any(|i| *i == a)
                    }) {
                        // All checks have passed so unlock the chain.
                        mutations::unlock_chain(txn, &author)?;
                        // Update ops to publish.
                        txn.execute("UPDATE DhtOp SET withhold_publish = NULL WHERE action_hash = :action_hash",
                        named_params! {
                            ":action_hash": this_cells_action_hash,
                            }
                        ).map_err(holochain_state::prelude::StateMutationError::from)?;
                        return Ok(Some(session_data));
                    }
                }
                SourceChainResult::Ok(None)
        }})
        .await?;

    if let Some(session_data) = result {
        // If all signatures are valid (above) and i signed then i must have
        // validated it previously so i now agree that i authored it.
        // TODO: perhaps this should be `authored_ops_to_dht_db`, i.e. the arc check should
        //       be performed, because we may not be an authority for these ops
        authored_ops_to_dht_db_without_check(
            this_cell_actions_op_basis_hashes
                .into_iter()
                .map(|(op_hash, _)| op_hash)
                .collect(),
            authored_db.into(),
            dht_db,
            &dht_db_cache,
        )
        .await?;
        integration_trigger.trigger(&"integrate countersigning_success");
        // Publish other signers agent activity ops to their agent activity authorities.
        for sa in signed_actions {
            let (action, signature) = sa.into();
            if *action.author() == author {
                continue;
            }
            let op = ChainOp::RegisterAgentActivity(signature, action);
            let basis = op.dht_basis();
            if let Err(e) = network.publish_countersign(false, basis, op.into()).await {
                tracing::error!(
                    "Failed to publish to other countersigners agent authorities because of: {:?}",
                    e
                );
            }
        }

        // Signal to the UI.
        // If there are no active connections this won't emit anything.
        let app_entry_hash = session_data.preflight_request.app_entry_hash.clone();
        signal
            .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                app_entry_hash,
            )))
            .ok();

        tracing::info!("Countersigning session complete for agent: {:?}", author);

        publish_trigger.trigger(&"publish countersigning_success");
    }

    Ok(true)
}

/// Publish to entry authorities, so they can gather all the signed
/// actions for this session and respond with a session complete.
pub async fn countersigning_publish(
    network: &HolochainP2pDna,
    op: ChainOp,
    _author: AgentPubKey,
) -> Result<(), ZomeCallResponse> {
    if let Some(enzyme) = op.enzymatic_countersigning_enzyme() {
        if let Err(e) = network
            .countersigning_session_negotiation(
                vec![enzyme.clone()],
                CountersigningSessionNegotiationMessage::EnzymePush(Box::new(op)),
            )
            .await
        {
            tracing::error!(
                "Failed to push countersigning ops to enzyme because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    } else {
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(true, basis, op.into()).await {
            tracing::error!(
                "Failed to publish to entry authorities for countersigning session because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    }
    Ok(())
}
