//! Countersigning workflow to maintain countersigning session state.

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::core::queue_consumer::{TriggerSender, WorkComplete};
use holo_hash::AgentPubKey;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Sender;
use tokio::task::AbortHandle;

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
pub(crate) use complete::force_publish_countersigning_session;
use holochain_keystore::MetaLairClient;
use holochain_state::chain_lock::get_chain_lock;
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

    pub fn get_countersigning_session_state(&self) -> Option<CountersigningSessionState> {
        self.inner
            .share_ref(|inner| Ok(inner.session.clone()))
            .unwrap()
    }

    pub fn remove_countersigning_session(&self) -> Option<CountersigningSessionState> {
        self.inner
            .share_mut(|inner, _| Ok(inner.session.take()))
            .unwrap()
    }
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    session: Option<CountersigningSessionState>,
    next_trigger: Option<NextTrigger>,
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn countersigning_workflow(
    space: Space,
    workspace: Arc<CountersigningWorkspace>,
    network: Arc<impl HolochainP2pDnaT>,
    keystore: MetaLairClient,
    cell_id: CellId,
    signal_tx: Sender<Signal>,
    self_trigger: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    tracing::debug!(
        "Starting countersigning workflow for agent {:?}, with a session? {:?}",
        cell_id.agent_pubkey(),
        workspace
            .inner
            .share_ref(|inner| Ok(inner.session.clone()))
            .unwrap()
    );

    // Clear trigger, if we need another one, it will be created later.
    workspace
        .inner
        .share_mut(|inner, _| {
            if let Some(next_trigger) = &mut inner.next_trigger {
                next_trigger.trigger_task.abort();
            }
            inner.next_trigger = None;
            Ok(())
        })
        .unwrap();

    // Ensure the workspace state knows about anything in the database on startup.
    refresh::refresh_workspace_state(
        &space,
        workspace.clone(),
        cell_id.clone(),
        signal_tx.clone(),
    )
    .await;

    // Abandon any sessions that have timed out.
    apply_timeout(&space, workspace.clone(), &cell_id, signal_tx.clone()).await;

    workspace
        .inner
        .share_mut(|inner, _| {
            inner.session.iter_mut().for_each(|state| {
                if let CountersigningSessionState::SignaturesCollected {
                    preflight_request,
                    signature_bundles,
                    resolution,
                } = state
                {
                    if signature_bundles.is_empty() && resolution.is_some() {
                        tracing::debug!("Countersigning session for agent {:?} has no valid signatures remaining and is a recovery session, returning to unknown state", cell_id.agent_pubkey());

                        // If we have no signatures and no resolution, then we need to try to recover
                        // the session.
                        *state = CountersigningSessionState::Unknown {
                            preflight_request: preflight_request.clone(),
                            resolution: resolution.clone().map(|r| SessionResolutionSummary {
                                attempts: r.attempts + 1,
                                last_attempt_at: Timestamp::now(),
                                outcomes: r.outcomes.clone(),
                                completion_attempts: r.completion_attempts + 1,
                            }),
                        };
                    }
                }
            });

            Ok(())
        })
        .unwrap();

    // If the session is in an unknown state, try to recover it.
    try_recover_failed_session(
        &space,
        workspace.clone(),
        network.clone(),
        &cell_id,
        &signal_tx,
        &self_trigger,
    )
    .await;

    let maybe_completed_session = workspace
        .inner
        .share_mut(|inner, _| {
            Ok(match &mut inner.session {
                Some(CountersigningSessionState::SignaturesCollected {
                    signature_bundles, ..
                }) => Some(std::mem::take(signature_bundles)),
                _ => None,
            })
        })
        .unwrap();

    if let Some(signature_bundles) = maybe_completed_session {
        for signature_bundle in signature_bundles {
            // Try to complete the session using this signature bundle.

            match complete::inner_countersigning_session_complete(
                space.clone(),
                network.clone(),
                keystore.clone(),
                cell_id.agent_pubkey().clone(),
                signature_bundle.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
            .await
            {
                Ok(Some(cs_entry_hash)) => {
                    // The session completed successfully this bundle, so we can remove the session
                    // from the workspace.
                    workspace
                        .inner
                        .share_mut(|inner, _| {
                            tracing::trace!("Countersigning session completed successfully, removing from the workspace for agent: {:?}", cell_id.agent_pubkey());
                            inner.session = None;
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
                    tracing::warn!("Rejected signature bundle for countersigning session for agent: {:?}: {:?}", cell_id.agent_pubkey(), signature_bundle);
                }
                Err(e) => {
                    tracing::error!(
                        "Error completing countersigning session for agent: {:?}: {:?}",
                        cell_id.agent_pubkey(),
                        e
                    );
                }
            }
        }
    }

    // At the end of the workflow, if we have a session still in progress, then schedule a
    // workflow run again at the end time.
    let maybe_end_time = workspace
        .inner
        .share_ref(|inner| {
            Ok(match &inner.session {
                Some(state) => match state {
                    CountersigningSessionState::Accepted(preflight_request)
                    | CountersigningSessionState::SignaturesCollected {
                        preflight_request, ..
                    } => Some(preflight_request.session_times.end),
                    CountersigningSessionState::Unknown { .. } => {
                        // Don't apply timeouts in the unknown state
                        None
                    }
                },
                None => None,
            })
        })
        .unwrap();

    tracing::trace!("End time: {:?}", maybe_end_time);

    if let Some(end_time) = maybe_end_time {
        reschedule_self(workspace, self_trigger, end_time);
    }

    Ok(WorkComplete::Complete)
}

async fn try_recover_failed_session(
    space: &Space,
    workspace: Arc<CountersigningWorkspace>,
    network: Arc<impl HolochainP2pDnaT + Sized>,
    cell_id: &CellId,
    signal_tx: &Sender<Signal>,
    self_trigger: &TriggerSender,
) {
    let maybe_session_in_unknown_state = workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .session
                .as_ref()
                .and_then(|session_state| match session_state {
                    CountersigningSessionState::Unknown {
                        preflight_request, ..
                    } => Some(preflight_request.clone()),
                    _ => None,
                }))
        })
        .unwrap();

    let mut remained_in_unknown_state = false;
    if let Some(preflight_request) = maybe_session_in_unknown_state {
        tracing::info!(
            "Countersigning session for agent {:?} is in an unknown state, attempting to resolve",
            cell_id.agent_pubkey()
        );
        match incomplete::inner_countersigning_session_incomplete(
            space.clone(),
            network.clone(),
            cell_id.agent_pubkey().clone(),
            preflight_request.clone(),
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
                workspace
                    .inner
                    .share_mut(|inner, _| {
                        tracing::trace!(
                            "Decision made for incomplete session, removing from workspace: {:?}",
                            cell_id.agent_pubkey()
                        );

                        inner.session = None;
                        Ok(())
                    })
                    .unwrap();

                signal_tx
                    .send(Signal::System(SystemSignal::AbandonedCountersigning(
                        preflight_request.app_entry_hash.clone(),
                    )))
                    .ok();
            }
            Ok((SessionCompletionDecision::Indeterminate, outcomes)) => {
                remained_in_unknown_state = true;
                tracing::info!(
                    "No automated decision could be reached for the current countersigning session: {:?}",
                    cell_id.agent_pubkey()
                );

                workspace.inner.share_mut(|inner, _| {
                    if let Some(session_state) = &mut inner.session {
                        if let CountersigningSessionState::Unknown {
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
                                    outcomes,
                                    ..Default::default()
                                });
                            }
                        } else {
                            tracing::error!("Countersigning session for agent {:?} was not in the expected state while trying to resolve it", cell_id.agent_pubkey());
                        }
                    } else {
                        tracing::error!("Countersigning session for agent {:?} was removed from the workspace while trying to resolve it", cell_id.agent_pubkey());
                    }

                    Ok(())
                }).unwrap();
            }
            Err(e) => {
                tracing::error!(
                    "Error cleaning up countersigning session for agent: {:?}: {:?}",
                    cell_id.agent_pubkey(),
                    e
                );
            }
        }
    }

    if remained_in_unknown_state {
        if let Ok(t) = Timestamp::now() + workspace.countersigning_resolution_retry_delay {
            reschedule_self(workspace, self_trigger.clone(), t);
        } else {
            tracing::error!("Failed to calculate next trigger time for countersigning workflow");
        }
    }
}

fn reschedule_self(
    workspace: Arc<CountersigningWorkspace>,
    self_trigger: TriggerSender,
    at_timestamp: Timestamp,
) {
    workspace
        .inner
        .share_mut(|inner, _| {
            if let Some(next_trigger) = &mut inner.next_trigger {
                next_trigger.replace_if_sooner(at_timestamp, self_trigger.clone());
            } else {
                inner.next_trigger = Some(NextTrigger::new(at_timestamp, self_trigger.clone()));
            }

            Ok(())
        })
        .unwrap();
}

async fn apply_timeout(
    space: &Space,
    workspace: Arc<CountersigningWorkspace>,
    cell_id: &CellId,
    signal_tx: Sender<Signal>,
) {
    let timed_out = workspace
        .inner
        .share_ref(|inner| {
            Ok(inner.session.as_ref().and_then(|session| {
                let expired = match session {
                    CountersigningSessionState::Accepted(preflight_request) => {
                        preflight_request.session_times.end < Timestamp::now()
                    }
                    CountersigningSessionState::SignaturesCollected {
                        preflight_request,
                        signature_bundles,
                        resolution,
                    } => {
                        // Only time out if all signatures have been tried and this is not a recovery state
                        // because recovery should be dealt with separately.
                        preflight_request.session_times.end < Timestamp::now()
                            && signature_bundles.is_empty()
                            && resolution.is_none()
                    }
                    _ => false,
                };

                if expired {
                    Some(session.preflight_request().clone())
                } else {
                    None
                }
            }))
        })
        .unwrap();

    if let Some(preflight_request) = timed_out {
        tracing::info!(
            "Countersigning session for agent {:?} has timed out, abandoning session",
            cell_id.agent_pubkey()
        );

        match force_abandon_session(space.clone(), cell_id.agent_pubkey(), &preflight_request).await
        {
            Ok(_) => {
                // Only once we've managed to remove the session do we remove the state for it.
                workspace
                    .inner
                    .share_mut(|inner, _| {
                        inner.session = None;
                        Ok(())
                    })
                    .unwrap();

                // Then let the client know.
                signal_tx
                    .send(Signal::System(SystemSignal::AbandonedCountersigning(
                        preflight_request.app_entry_hash,
                    )))
                    .ok();
            }
            Err(e) => {
                tracing::error!(
                    "Error abandoning countersigning session for agent: {:?}: {:?}",
                    cell_id.agent_pubkey(),
                    e
                );
            }
        }
    }
}

pub(crate) async fn force_abandon_session(
    space: Space,
    author: &AgentPubKey,
    preflight_request: &PreflightRequest,
) -> SourceChainResult<()> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

    let abandon_fingerprint = preflight_request.fingerprint()?;

    let maybe_session_data = authored_db
        .write_async({
            let author = author.clone();
            move |txn| current_countersigning_session(txn, Arc::new(author.clone()))
        })
        .await?;

    match maybe_session_data {
        Some((cs_action, cs_entry_hash, x))
            if x.preflight_request.fingerprint()? == abandon_fingerprint =>
        {
            tracing::info!("There is a committed session to remove for: {:?}", author);
            abandon_session(
                authored_db,
                author.clone(),
                cs_action.action().clone(),
                cs_entry_hash,
            )
            .await?;
        }
        _ => {
            // There is no matching, committed session but there may be a lock to remove
            authored_db
                .write_async({
                    let author = author.clone();
                    move |txn| {
                        let chain_lock = get_chain_lock(txn, &author)?;

                        match chain_lock {
                            Some(lock) if lock.subject() == abandon_fingerprint => {
                                unlock_chain(txn, &author)
                            }
                            _ => {
                                tracing::warn!(
                                    "No matching session or lock to remove for: {:?}",
                                    author
                                );
                                Ok(())
                            }
                        }
                    }
                })
                .await?;
        }
    }

    Ok(())
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

// TODO unify with the other mechanisms for re-triggering. This is currently working around
//      a performance issue with WorkComplete::Incomplete but is similar to the loop logic that
//      other workflows use - the difference being that this workflow varies the loop delay.
struct NextTrigger {
    trigger_at: Timestamp,
    trigger_task: AbortHandle,
}

impl NextTrigger {
    fn new(trigger_at: Timestamp, trigger_sender: TriggerSender) -> Self {
        let delay = Self::calculate_delay(&trigger_at);

        let trigger_task = Self::start_trigger_task(delay, trigger_sender);

        Self {
            trigger_at,
            trigger_task,
        }
    }

    fn replace_if_sooner(&mut self, trigger_at: Timestamp, trigger_sender: TriggerSender) {
        // If the current trigger has expired, or the new one is sooner, then replace the
        // current trigger.
        if self.trigger_at < Timestamp::now() || trigger_at < self.trigger_at {
            let new_delay = Self::calculate_delay(&trigger_at);
            self.trigger_task.abort();
            self.trigger_at = trigger_at;
            self.trigger_task = Self::start_trigger_task(new_delay, trigger_sender);
        }
    }

    fn calculate_delay(trigger_at: &Timestamp) -> Duration {
        match trigger_at
            .checked_difference_signed(&Timestamp::now())
            .map(|d| d.to_std())
        {
            Some(Ok(d)) => d,
            _ => Duration::from_millis(100),
        }
    }

    fn start_trigger_task(delay: Duration, trigger_sender: TriggerSender) -> AbortHandle {
        tracing::trace!("Scheduling countersigning workflow in: {:?}", delay);
        tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            trigger_sender.trigger(&"next trigger");
        })
        .abort_handle()
    }
}
