use crate::conductor::space::Space;
use crate::core::workflow::countersigning_workflow::SessionState;
use holochain_sqlite::db::ReadAccess;
use holochain_state::chain_lock::get_chain_lock;
use holochain_state::mutations::unlock_chain;
use holochain_state::prelude::{
    current_countersigning_session, CurrentCountersigningSessionOpt, SourceChainResult,
};
use holochain_types::prelude::{Signal, SystemSignal};
use holochain_zome_types::cell::CellId;
use std::collections::{HashMap, HashSet};
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
pub async fn refresh_workspace_state(space: &Space, cell_id: CellId, signal: Sender<Signal>) {
    tracing::debug!(
        "Refreshing countersigning workspace state for {:?}",
        cell_id
    );
    let workspace = &space.countersigning_workspace;

    // These are all the agents that the conductor is aware of for this the DNA this workflow is running in.
    //
    // We don't want to keep the entire space locked for writes, so just get the agents and release the lock.
    // We can then lock each agent individually.
    let agents = {
        space
            .authored_dbs
            .lock()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };

    // These are all the agents who currently have a session registered.
    let sessions_registered_for_agents = workspace
        .inner
        .share_ref(|inner| Ok(inner.sessions.keys().cloned().collect::<HashSet<_>>()))
        .unwrap();

    let mut locked_for_agents = HashSet::new();

    // Run through all the agents that the conductor is aware of and check if they have a chain lock
    // or a countersigning session stored.
    for agent in agents {
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
                locked_for_agents.insert(agent.clone());

                // Try to retrieve the current countersigning session. If we can't then we have lost
                // the state of the session and need to unlock the chain.
                // This might happen if we were in the coordination phase of countersigning and the
                // conductor restarted.
                let query_session_and_maybe_unlock_result = authored_db
                    .write_async({
                        let agent = agent.clone();
                        let sessions_registered_for_agents = sessions_registered_for_agents.clone();
                        move |txn| -> SourceChainResult<(CurrentCountersigningSessionOpt, bool)> {
                            let maybe_current_session =
                                current_countersigning_session(txn, Arc::new(agent.clone()))?;
                            tracing::info!("Found session? {:?}", maybe_current_session);

                            // If we've not made a commit and the entry hasn't been committed then
                            // there is no way to recover the session.
                            // We also can't have published a signature yet, so it's safe to unlock
                            // the chain here and abandon the session.
                            if maybe_current_session.is_none() && !sessions_registered_for_agents.contains(&agent) {
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
                            // Not super important to remove this from the list. We can only get here if
                            // the session was not in the workspace.
                            locked_for_agents.remove(&agent);
                        }

                        match maybe_current_session {
                            Some((_, _, session_data)) => {
                                tracing::info!(
                                    "Found a countersigning session for agent {:?}",
                                    agent
                                );

                                // Any locked chains, that aren't registered in the workspace, need to be added.
                                // They have to go in as `Unknown` because we don't know the state of the session.
                                workspace
                                    .inner
                                    .share_mut(|inner, _| {
                                        inner.sessions.entry(agent.clone()).or_insert(
                                            SessionState::Unknown {
                                                preflight_request: session_data
                                                    .preflight_request()
                                                    .clone(),
                                                resolution: None,
                                            },
                                        );

                                        Ok(())
                                    })
                                    .unwrap();
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
    }

    // Any sessions that are in the workspace but not locked need to be removed.
    let dropped_sessions = workspace
        .inner
        .share_mut(|inner, _| {
            let (keep, drop) = inner
                .sessions
                .drain()
                .partition::<HashMap<_, _>, _>(|(agent, _)| locked_for_agents.contains(agent));
            inner.sessions = keep;
            Ok(drop)
        })
        .unwrap();

    // This is expected to happen when the countersigning commit fails validation. The chain gets
    // unlocked, and we are just cleaning up state here.
    for (agent, session_state) in dropped_sessions {
        tracing::debug!("Countersigning session for agent {:?} is in the workspace but the chain is not locked, removing from workspace", agent);

        let entry_hash = session_state.session_app_entry_hash();

        // Best effort attempt to let clients know that the session has been abandoned.
        signal
            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                entry_hash.clone(),
            )))
            .ok();
    }
}
