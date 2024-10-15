use crate::conductor::space::Space;
use crate::core::workflow::countersigning_workflow::{
    CountersigningSessionState, CountersigningWorkspace, ResolutionRequiredReason,
    SessionResolutionSummary,
};
use holochain_sqlite::db::ReadAccess;
use holochain_state::chain_lock::get_chain_lock;
use holochain_state::mutations::unlock_chain;
use holochain_state::prelude::{
    current_countersigning_session, CurrentCountersigningSessionOpt, SourceChainResult,
};
use holochain_types::prelude::{Signal, SystemSignal};
use holochain_zome_types::cell::CellId;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

/// Resolves the various states that the system can find itself in when operating a countersigning session.
///
/// As much as possible, the system does try to avoid needing correction, but there are some important
/// exceptions where being able to recover to a known state is important.
///
/// 1. The session has been accepted, and the chain is locked, but the conductor restarted so the
///    session is not tracked in the workspace.
/// 2. The countersigning entry failed validation, so the chain has been unlocked, but the session
///    is still in the workspace.
/// 3. The countersigning entry has been committed and the chain is still locked, but the conductor
///    has restarted, so the session is not in the workspace.
pub async fn refresh_workspace_state(
    space: &Space,
    workspace: Arc<CountersigningWorkspace>,
    cell_id: CellId,
    signal: Sender<Signal>,
) {
    tracing::debug!(
        "Refreshing countersigning workspace state for {:?}",
        cell_id
    );

    // Whether there is a session currently registered in the workspace.
    let session_registered_for_agent = workspace
        .inner
        .share_ref(|inner| Ok(inner.session.is_some()))
        .unwrap();

    let mut locked_for_agent = false;

    let agent = cell_id.agent_pubkey().clone();
    if let Ok(authored_db) = space.get_or_create_authored_db(agent.clone()) {
        let lock = authored_db
            .read_async({
                let agent = agent.clone();
                move |txn| get_chain_lock(&txn, &agent)
            })
            .await
            .ok()
            .flatten();

        // If the chain is locked, then we need to check the session state.
        if lock.is_some() {
            locked_for_agent = true;

            // Try to retrieve the current countersigning session. If we can't then we have lost
            // the state of the session and need to unlock the chain.
            // This might happen if we were in the coordination phase of countersigning and the
            // conductor restarted.
            let query_session_and_maybe_unlock_result = authored_db
                    .write_async({
                        let agent = agent.clone();
                        move |txn| -> SourceChainResult<(CurrentCountersigningSessionOpt, bool)> {
                            let maybe_current_session =
                                current_countersigning_session(txn, Arc::new(agent.clone()))?;
                            tracing::trace!("Current session: {:?}", maybe_current_session);

                            // If we've not made a commit and the entry hasn't been committed then
                            // there is no way to recover the session.
                            // We also can't have published a signature yet, so it's safe to unlock
                            // the chain here and abandon the session.
                            if maybe_current_session.is_none() && !session_registered_for_agent {
                                tracing::info!("Found a chain lock, but no corresponding countersigning session or workspace reference. Unlocking chain for agent {:?}", agent);
                                unlock_chain(txn, &agent)?;
                                Ok((None, true))
                            } else {
                                Ok((maybe_current_session, false))
                            }
                        }
                    })
                    .await;

            match query_session_and_maybe_unlock_result {
                Ok((maybe_current_session, unlocked)) => {
                    if unlocked {
                        locked_for_agent = false;

                        // Ideally, we'd signal here, but we don't know the app entry hash.
                    }

                    match maybe_current_session {
                        Some((_, _, session_data)) => {
                            if !session_registered_for_agent {
                                // The chain is locked but the session isn't registered in the workspace.
                                // It needs to be added in with the `Unknown` state because we don't
                                // know the state of the session.
                                workspace
                                    .inner
                                    .share_mut(|inner, _| {
                                        inner.session = Some(CountersigningSessionState::Unknown {
                                            preflight_request: session_data
                                                .preflight_request()
                                                .clone(),
                                            resolution: SessionResolutionSummary {
                                                required_reason: ResolutionRequiredReason::Unknown,
                                                ..Default::default()
                                            },
                                            force_publish: false,
                                        });

                                        Ok(())
                                    })
                                    .unwrap();
                            }
                        }
                        None => {
                            // No session entry was found. This can happen if the chain is locked for
                            // the session accept but no commit has been done yet. Either the author
                            // will commit or the session will time out. Nothing to be done here!
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Error querying countersigning chain state for agent {:?}: {:?}",
                        agent,
                        e
                    );
                }
            }
        }
    }

    // If there is a session in the workspace but the chain is not locked, then we need to remove
    // the session from the workspace.
    let maybe_dropped = workspace
        .inner
        .share_mut(|inner, _| {
            let mut out = None;
            if let Some(session) = &inner.session {
                if !locked_for_agent {
                    out = Some(session.session_app_entry_hash().clone());
                }
            }

            if out.is_some() {
                inner.session = None;
            }

            Ok(out)
        })
        .unwrap();

    // This is expected to happen when the countersigning commit fails validation. The chain gets
    // unlocked, and we are just cleaning up state here.
    if let Some(entry_hash) = maybe_dropped {
        tracing::debug!("Countersigning session for agent {:?} is in the workspace but the chain is not locked, removing from workspace", agent);

        // Best effort attempt to let clients know that the session has been abandoned.
        signal
            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                entry_hash.clone(),
            )))
            .ok();
    }
}
