//! Countersigning workflow to maintain countersigning session state.

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::core::queue_consumer::{TriggerSender, WorkComplete};
use holo_hash::AgentPubKey;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::KitsuneResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Sender;

/// Accept handler for starting countersigning sessions.
mod accept;

/// Inner workflow for resolving an incomplete countersigning session.
mod incomplete;

/// Inner workflow for completing a countersigning session based on received signatures.
mod complete;

/// State integrity function to ensure that the database and the workspace are in sync.
mod refresh;

/// Success handler for receiving signature bundles from the network.
mod success;

#[cfg(test)]
mod tests;

pub(crate) use accept::accept_countersigning_request;
pub(crate) use success::countersigning_success;

/// Countersigning workspace to hold session state.
#[derive(Clone)]
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
    countersigning_resolution_retry_delay: Duration,
}

impl CountersigningWorkspace {
    /// Create a new countersigning workspace.
    pub fn new(countersigning_resolution_retry_delay: Duration) -> Self {
        Self {
            inner: Default::default(),
            countersigning_resolution_retry_delay,
        }
    }

    pub async fn get_countersigning_session_state(
        &self,
        agent_key: &AgentPubKey,
    ) -> KitsuneResult<Option<CounterSigningSessionState>> {
        self.inner.share_ref(|inner| {
            println!("there are {:?} sessions here", inner.sessions.len());
            Ok(inner.sessions.get(agent_key).map(|s| s.clone()))
        })
    }
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    sessions: HashMap<AgentPubKey, CounterSigningSessionState>,
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn countersigning_workflow(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    cell_id: CellId,
    signal_tx: Sender<Signal>,
    self_trigger: TriggerSender,
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

    refresh::refresh_workspace_state(&space, cell_id.clone(), signal_tx.clone()).await;

    // Apply timeouts by moving accepted sessions to unknown state if their end time is in the past.
    space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            inner
                .sessions
                .iter_mut()
                .for_each(|(author, session_state)| {
                    if let CounterSigningSessionState::Accepted(request) = session_state {
                        if request.session_times.end < Timestamp::now() {
                            tracing::info!(
                                "Session for agent {:?} has timed out, moving to unknown state",
                                author
                            );
                            *session_state = CounterSigningSessionState::Unknown {
                                preflight_request: request.clone(),
                                resolution: None,
                            };
                        }
                    }
                });

            Ok(())
        })
        .unwrap();

    // Find sessions that are in an unknown state, we need to try to resolve those.
    let sessions_in_unknown_state = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    CounterSigningSessionState::Unknown { .. } => Some(author.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    let mut remaining_sessions_in_unknown_state = 0;
    for author in sessions_in_unknown_state {
        tracing::info!(
            "Countersigning session for agent {:?} is in an unknown state, attempting to resolve",
            author
        );
        match incomplete::inner_countersigning_session_incomplete(
            space.clone(),
            network.clone(),
            author.clone(),
        )
        .await
        {
            Ok((SessionCompletionDecision::Complete(_), _)) => {
                // No need to do anything here. Signatures were found which may be able to complete
                // the session but the session isn't actually complete yet. We need to let the
                // workflow re-run and try those signatures.
            }
            Ok((SessionCompletionDecision::Abandoned, _)) => {
                // The session state has been resolved, so we can remove it from the workspace.
                let removed_session = space
                    .countersigning_workspace
                    .inner
                    .share_mut(|inner, _| {
                        tracing::trace!(
                            "Decision made for incomplete session, removing from workspace: {:?}",
                            author
                        );
                        let removed_session = inner.sessions.remove(&author);
                        Ok(removed_session)
                    })
                    .unwrap();

                if let Some(removed_session) = removed_session {
                    let entry_hash = removed_session.session_app_entry_hash().clone();

                    signal_tx
                        .send(Signal::System(SystemSignal::AbandonedCountersigning(
                            entry_hash,
                        )))
                        .ok();
                }
            }
            Ok((SessionCompletionDecision::Indeterminate, outcomes)) => {
                remaining_sessions_in_unknown_state += 1;
                tracing::info!(
                    "No automated decision could be reached for the current countersigning session: {:?}",
                    author
                );

                space.countersigning_workspace.inner.share_mut(|inner, _| {
                    match inner.sessions.entry(cell_id.agent_pubkey().clone()) {
                        std::collections::hash_map::Entry::Occupied(mut entry) => {
                            let session_state = entry.get_mut();
                            if let CounterSigningSessionState::Unknown {
                                resolution,
                                ..
                            } = session_state
                            {
                                if let Some(resolution) = resolution {
                                    resolution.attempts += 1;
                                    resolution.last_attempt_at = Timestamp::now();
                                    resolution.outcomes = outcomes;
                                } else {
                                    *resolution = Some(SessionResolutionSummary {
                                        attempts: 1,
                                        last_attempt_at: Timestamp::now(),
                                        outcomes,
                                    });
                                }
                            } else {
                                tracing::error!("Countersigning session for agent {:?} was not in the expected state while trying to resolve it", author);
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(_) => {
                            tracing::error!("Countersigning session for agent {:?} was removed from the workspace while trying to resolve it", author);
                        }
                    };

                    Ok(())
                }).unwrap();
            }
            Err(e) => {
                tracing::error!(
                    "Error cleaning up countersigning session for agent: {:?}: {:?}",
                    author,
                    e
                );
            }
        }
    }

    if remaining_sessions_in_unknown_state > 0 {
        tokio::task::spawn({
            let self_trigger = self_trigger.clone();
            async move {
                tokio::time::sleep(
                    space
                        .countersigning_workspace
                        .countersigning_resolution_retry_delay,
                )
                .await;
                self_trigger.trigger(&"unknown_session_state_retry");
            }
        });
    }

    let maybe_completed_sessions = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    CounterSigningSessionState::SignaturesCollected {
                        signature_bundles, ..
                    } => Some((author.clone(), signature_bundles.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    for (author, signatures) in maybe_completed_sessions {
        for signature_bundle in signatures {
            // Try to complete the session using this signature bundle.

            match complete::inner_countersigning_session_complete(
                space.clone(),
                network.clone(),
                author.clone(),
                signature_bundle.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
            .await
            {
                Ok(Some(cs_entry_hash)) => {
                    // The session completed successfully this bundle, so we can remove the session
                    // from the workspace.
                    space
                        .countersigning_workspace
                        .inner
                        .share_mut(|inner, _| {
                            tracing::trace!("Countersigning session completed successfully, removing from the workspace for agent: {:?}", author);
                            inner.sessions.remove(&author);
                            Ok(())
                        })
                        .unwrap();

                    // Signal to the UI.
                    // If there are no active connections this won't emit anything.
                    signal_tx
                        .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                            cs_entry_hash,
                        )))
                        .ok();

                    break;
                }
                Ok(None) => {
                    tracing::warn!("Rejected signature bundle for countersigning session for agent: {:?}: {:?}", author, signature_bundle);
                }
                Err(e) => {
                    tracing::error!(
                        "Error completing countersigning session for agent: {:?}: {:?}",
                        author,
                        e
                    );
                }
            }
        }
    }

    // At the end of the workflow, if we have any sessions still in progress, then schedule a
    // workflow run for the one that will finish soonest.
    let maybe_earliest_finish = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .values()
                .filter_map(|s| {
                    match s {
                        CounterSigningSessionState::Accepted(request) => {
                            Some(request.session_times.end)
                        }
                        _ => {
                            // Could be waiting for more signatures, or in an unknown state.
                            None
                        }
                    }
                })
                .min())
        })
        .unwrap();

    // TODO This risks building up duplicate triggers
    if let Some(earliest_finish) = maybe_earliest_finish {
        let delay = match (earliest_finish - Timestamp::now()).map(|d| d.to_std()) {
            Ok(Ok(d)) => d,
            _ => Duration::from_millis(100),
        };
        tracing::debug!("Countersigning workflow will run again in {:?}", delay);
        tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            self_trigger.trigger(&"retrigger_expiry_check");
        });
    }

    Ok(WorkComplete::Complete)
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

/// Abandon a countersigning session.
async fn abandon_session(
    authored_db: DbWrite<DbKindAuthored>,
    author: AgentPubKey,
    cs_action: Action,
    cs_entry_hash: EntryHash,
) -> StateMutationResult<()> {
    authored_db
        .write_async(move |txn| -> StateMutationResult<()> {
            // Do the dangerous thing and remove the countersigning session.
            remove_countersigning_session(txn, cs_action, cs_entry_hash)?;

            // Once the session is removed we can unlock the chain.
            unlock_chain(txn, &author)?;

            Ok(())
        })
        .await?;

    Ok(())
}
