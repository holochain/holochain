use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::workflow::countersigning_workflow::{
    success, SessionCompletionDecision, SessionResolutionOutcome, NUM_AUTHORITIES_TO_QUERY,
};
use crate::core::workflow::{countersigning_workflow, WorkflowResult};
use crate::prelude::{Entry, PreflightRequest, RecordEntry};
use either::Either;
use holo_hash::AgentPubKey;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::{
    current_countersigning_session, CurrentCountersigningSessionOpt, SourceChainResult,
};
use holochain_types::activity::ChainItems;
use holochain_zome_types::entry::GetOptions;
use holochain_zome_types::prelude::{
    ChainQueryFilter, ChainQueryFilterRange, ChainStatus, SignedAction,
};
use itertools::Itertools;
use std::sync::Arc;

/// Resolve an incomplete countersigning session.
///
/// This function is responsible for deciding what action to take when a countersigning session
/// has failed to complete.
///
/// The function returns true if the session state has been cleaned up and the chain has been unlocked.
/// Otherwise, the function returns false and the cleanup needs to be retried to resolved manually.
pub async fn inner_countersigning_session_incomplete(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    author: AgentPubKey,
    preflight_request: PreflightRequest,
) -> WorkflowResult<(SessionCompletionDecision, Vec<SessionResolutionOutcome>)> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

    let maybe_current_session = authored_db
        .read_async({
            let author = author.clone();
            move |txn| -> SourceChainResult<CurrentCountersigningSessionOpt> {
                let maybe_current_session =
                    current_countersigning_session(&txn, Arc::new(author.clone()))?;

                Ok(maybe_current_session)
            }
        })
        .await?;

    if maybe_current_session.is_none() {
        tracing::error!("Countersigning session was in an unknown state but no session entry was found. Holochain is only meant to enter this state when there is an entry to remove and won't recover: {:?}", author);
        return Ok((
            SessionCompletionDecision::Indeterminate,
            Vec::with_capacity(0),
        ));
    }

    // Now things get more complicated. We have a countersigning entry on our chain but the session
    // is in a bad state. We need to figure out what the session state is and how to resolve it.

    let (cs_record, cs_entry_hash, session_data) = maybe_current_session.unwrap();

    let found_fingerprint = session_data.preflight_request().fingerprint()?;
    let expected_fingerprint = preflight_request.fingerprint()?;
    if found_fingerprint != expected_fingerprint {
        tracing::error!("Countersigning session {:?} was in an unknown state but the session entry found was {:?} which does not match: {:?}", preflight_request, session_data.preflight_request(), author);
        return Ok((
            SessionCompletionDecision::Indeterminate,
            Vec::with_capacity(0),
        ));
    }

    // We need to find out what state the other signing agents are in.
    // TODO Note that we are ignoring the optional signing agents here - that's something we can figure out later because it's not clear what it means for them
    //      to be optional.
    //      Possibly get more options for collecting signatures by asking optional signers
    let other_signing_agents = session_data
        .signing_agents()
        .filter(|a| **a != author)
        .collect::<Vec<_>>();

    let cascade = CascadeImpl::empty().with_network(network, space.cache_db.clone());

    let get_activity_options = GetActivityOptions {
        include_warrants: false, // TODO document that apps should consider checking for warrants before completing preflight
        include_valid_activity: true,
        include_full_records: true,
        get_options: GetOptions::network(),
        // We're going to be potentially running quite a lot of these requests, so set the timeout reasonably low.
        timeout_ms: Some(10_000),
        ..Default::default()
    };

    let mut by_agent_decisions = Vec::new();
    let mut resolution_outcomes = Vec::new();

    for agent in other_signing_agents {
        let agent_state = session_data.agent_state_for_agent(agent)?;

        // Query for the other agent's activity.
        // We only need a small sample to determine whether they've committed the session entry
        // or something else in the sequence number after their declared chain top.
        let filter = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                *agent_state.action_seq() + 1,
                agent_state.action_seq() + 1,
            ))
            .include_entries(true);

        let mut authority_decisions = Vec::new();

        // Try multiple times to get the activity for this agent.
        // Ideally, each request will go to a different authority, so we don't have to trust a single
        // authority. However, that is not guaranteed.
        // TODO Should not need a loop here. The cascade/network should handle doing multiple
        //      requests. It's partially implemented but currently only sends to one authority.
        for _ in 0..NUM_AUTHORITIES_TO_QUERY {
            let activity_result = cascade
                .get_agent_activity(agent.clone(), filter.clone(), get_activity_options.clone())
                .await;

            let decision = match activity_result {
                Ok(activity) => {
                    match activity.status {
                        ChainStatus::Valid(ref head) => {
                            tracing::trace!("Agent {:?} has a valid chain: {:?}", agent, head);
                        }
                        ChainStatus::Empty => {
                            tracing::debug!(
                                "Agent {:?} has not published any further actions yet",
                                agent
                            );
                            authority_decisions.push(SessionCompletionDecision::Indeterminate);
                            continue;
                        }
                        status => {
                            tracing::info!(
                                "Agent {:?} has an invalid chain state for resolution: {:?}",
                                agent,
                                status
                            );
                            // Don't try to reason about this agent's state if the chain is invalid or forked.
                            continue;
                        }
                    }

                    match activity.valid_activity {
                        ChainItems::Full(ref items) => {
                            if items.is_empty() {
                                // The agent has not published any new activity
                                SessionCompletionDecision::Indeterminate
                            } else if items.len() > 1 {
                                tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
                                // This is an entirely unexpected response. We should not attempt further processing.
                                // Even with a protocol mismatch of some kind, we should not have more than one item returned.
                                return Ok((
                                    SessionCompletionDecision::Failed,
                                    Vec::with_capacity(0),
                                ));
                            } else {
                                let maybe_countersigning_record = &items[0];
                                match &maybe_countersigning_record.entry {
                                    RecordEntry::Present(Entry::CounterSign(cs, _)) => {
                                        // Check that this is the same session, and not some other session on the other agent's chain.
                                        if cs.preflight_request == session_data.preflight_request {
                                            tracing::debug!("Agent {:?} has a countersigning entry for the same session", agent);
                                            // This agent appears to have completed the countersigning session.
                                            // Collect the signed action to use as a signature for completing the session for our agent.
                                            SessionCompletionDecision::Complete(Box::new(
                                                maybe_countersigning_record.signed_action.clone(),
                                            ))
                                        } else {
                                            // This is evidence that the other agent has put something else on their chain.
                                            // Specifically, some other countersigning session.
                                            SessionCompletionDecision::Abandoned
                                        }
                                    }
                                    RecordEntry::Present(_) => {
                                        // Anything else on the chain is evidence that the agent did not complete the countersigning.
                                        tracing::debug!(
                                            "Agent {:?} has a non-countersigning entry",
                                            agent
                                        );
                                        SessionCompletionDecision::Abandoned
                                    }
                                    RecordEntry::Hidden => {
                                        // Hidden countersigning entries don't make sense. Two parties are agreeing to something
                                        // so at least two people should be able to see the content. Hidden entries are visible
                                        // only to their author. This comment is being left here in case somebody decides to add
                                        // a concept of hidden countersigning entries in the future. You will need to modify
                                        // the logic here!
                                        SessionCompletionDecision::Abandoned
                                    }
                                    RecordEntry::NA => {
                                        // This wouldn't be the case for a countersigning entry so this is evidence that
                                        // the agent has put something else on their chain.
                                        SessionCompletionDecision::Abandoned
                                    }
                                    RecordEntry::NotStored => {
                                        // This case is not determinate. The agent activity authority might
                                        // have the action but not the entry yet. We can't make a decision here.
                                        // In this case, we will also have tried to ask a record authority for
                                        // this missing entry.
                                        SessionCompletionDecision::Indeterminate
                                    }
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
                            // This should not happen, the authority did not understand our request for some reason.
                            // This could be caused by a mismatch between conductors. Attempt no further processing
                            // and require a new attempt that talks to peers who can answer hte query as we exepct.
                            return Ok((SessionCompletionDecision::Failed, Vec::with_capacity(0)));
                        }
                    }
                }
                e => {
                    tracing::warn!(
                        "Failed to get activity for agent {:?} because of: {:?}",
                        agent,
                        e
                    );
                    // Some error getting agent activity from this authority, don't treat this as
                    // indeterminate, but require a new attempt
                    return Ok((SessionCompletionDecision::Failed, Vec::with_capacity(0)));
                }
            };

            authority_decisions.push(decision);
        }

        resolution_outcomes.push(SessionResolutionOutcome {
            agent: agent.clone(),
            decisions: authority_decisions.clone(),
        });

        if authority_decisions.len() < NUM_AUTHORITIES_TO_QUERY {
            // We are requiring all the authorities to agree, so if we don't have enough responses
            // then we can't make a decision.
            // That is likely to make the resolution process slower, but it's more likely to be correct.
            // NOTE: at the moment, since we're calling `get_agent_activity` without a target,
            //       all the responses could have come from the same authority.
            tracing::warn!(
                "Not enough responses to make a decision for agent {:?}. Have {}/{}",
                agent,
                authority_decisions.len(),
                NUM_AUTHORITIES_TO_QUERY
            );
            by_agent_decisions.push(SessionCompletionDecision::Indeterminate);
            continue;
        }

        if authority_decisions
            .iter()
            .all(|d| matches!(*d, SessionCompletionDecision::Complete(_)))
        {
            tracing::debug!(
                "Authorities agree that agent {:?} has completed the session",
                agent
            );
            // Safe to access without bounds check because we've done a size check above.
            by_agent_decisions.push(authority_decisions[0].clone());
        } else if authority_decisions
            .iter()
            .all(|d| *d == SessionCompletionDecision::Abandoned)
        {
            tracing::debug!(
                "Authorities agree that agent {:?} has abandoned the session",
                agent
            );
            by_agent_decisions.push(SessionCompletionDecision::Abandoned);
        } else {
            // The decisions are either mixed or indeterminate. We can't make a decision so
            // collapse the responses to indeterminate.
            by_agent_decisions.push(SessionCompletionDecision::Indeterminate);
        }
    }

    let (mut signatures, abandoned): (Vec<SignedAction>, Vec<_>) = by_agent_decisions
        .into_iter()
        .filter(|d| *d != SessionCompletionDecision::Indeterminate)
        .partition_map(|d| match d {
            SessionCompletionDecision::Complete(sah) => Either::Left((*sah).into()),
            SessionCompletionDecision::Abandoned => Either::Right(()),
            _ => unreachable!(),
        });

    // Add our own action to the list of signatures.
    tracing::debug!(
        "Session resolution found {}/{} signatures and {}/{} abandoned",
        signatures.len(),
        session_data.preflight_request().signing_agents.len() - 1,
        abandoned.len(),
        session_data.preflight_request().signing_agents.len() - 1
    );

    signatures.push(cs_record.signed_action.clone().into());

    if signatures.len() == session_data.preflight_request().signing_agents.len() {
        // We have all the signatures we need to complete the session. We can complete the session
        // without further action from our agent.
        // This is equivalent to receiving a signature bundle from a witness.
        tracing::debug!(
            "Submitting signature bundle to complete countersigning session for agent {:?}",
            author
        );

        // Note that we discard signals here! This function is normally run from a network request
        // where it will need to trigger the workflow to run after adding signatures into the
        // workspace state. Here, we've been called by the workflow, so we don't need to trigger.
        success::countersigning_success(space, author.clone(), signatures, TriggerSender::new().0)
            .await;

        // We haven't actually succeeded at this point, because the workflow will need to run again
        // to try and process the new signature bundle. We communicate completion here but the
        // caller must not change the session state based on this response.
        return Ok((
            SessionCompletionDecision::Complete(cs_record.signed_action.clone().into()),
            Vec::with_capacity(0),
        ));
    } else if abandoned.len() == session_data.preflight_request().signing_agents.len() - 1 {
        // We have evidence from all the authorities that we contacted, that all the other agents
        // in this session have abandoned the session. We can abandon the session too.
        // Note that for a two party session, this just means one other agent!
        tracing::debug!("All other agents have abandoned the countersigning session, abandoning session for agent {:?}", author);
        countersigning_workflow::abandon_session(
            authored_db,
            author.clone(),
            cs_record.action().clone(),
            cs_entry_hash,
        )
        .await?;
        return Ok((SessionCompletionDecision::Abandoned, Vec::with_capacity(0)));
    }

    // Otherwise, we need to be cautious. Expose the current state of the session to the user so that
    // they can force a decision if they wish. However, Holochain cannot make a decision at this point
    // because we aren't absolutely sure that the session is complete or abandoned.

    tracing::info!(
        "Countersigning session state for agent {:?} is indeterminate, will retry later",
        author
    );
    Ok((
        SessionCompletionDecision::Indeterminate,
        resolution_outcomes,
    ))
}
