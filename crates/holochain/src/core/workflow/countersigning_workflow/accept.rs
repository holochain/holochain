use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::workflow::countersigning_workflow::SessionState;
use crate::core::workflow::{WorkflowError, WorkflowResult};
use crate::prelude::{PreflightRequest, PreflightRequestAcceptance, PreflightResponse, Signature};
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use kitsune_p2p_types::KitsuneError;

/// Accept a countersigning session.
///
/// This will register the session in the workspace, lock the agent's source chain and build the
/// pre-flight response.
pub async fn accept_countersigning_request(
    space: Space,
    keystore: MetaLairClient,
    author: AgentPubKey,
    request: PreflightRequest,
    countersigning_trigger: TriggerSender,
) -> WorkflowResult<PreflightRequestAcceptance> {
    // Find the index of our agent in the list of signing agents.
    let agent_index = match request
        .signing_agents
        .iter()
        .position(|(agent, _)| agent == &author)
    {
        Some(agent_index) => agent_index as u8,
        None => return Ok(PreflightRequestAcceptance::UnacceptableAgentNotFound),
    };

    // Take out a lock on our source chain and build our current state to include in the pre-flight
    // response.
    let source_chain = space.source_chain(keystore.clone(), author.clone()).await?;
    let countersigning_agent_state = source_chain
        .accept_countersigning_preflight_request(request.clone(), agent_index)
        .await?;

    // Create a signature for the pre-fight response, so that other agents can verify that the
    // acceptance really came from us.
    let signature: Signature = match keystore
        .sign(
            author.clone(),
            PreflightResponse::encode_fields_for_signature(&request, &countersigning_agent_state)?
                .into(),
        )
        .await
    {
        Ok(signature) => signature,
        Err(e) => {
            // Attempt to unlock the chain again.
            // If this fails the chain will remain locked until the session end time.
            // But also we're handling a keystore error already, so we should return that.
            if let Err(unlock_error) = source_chain.unlock_chain().await {
                tracing::error!(?unlock_error);
            }

            return Err(WorkflowError::other(e));
        }
    };

    // At this point the chain has been locked, and we are in a countersigning session. Store the
    // session request in the workspace.
    if let Err(_) = space.countersigning_workspace.inner.share_mut(|inner, _| {
        match inner.sessions.entry(author.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(KitsuneError::other("Session already exists"))
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SessionState::Accepted(request.clone()));
                Ok(())
            }
        }
    }) {
        // This really shouldn't happen. The chain lock is the primary state and that should be in place here.
        return Ok(PreflightRequestAcceptance::AnotherSessionIsInProgress);
    };

    // Kick off the countersigning workflow and let it figure out what actions to take.
    tracing::debug!("Accepted countersigning session, triggering countersigning workflow");
    countersigning_trigger.trigger(&"accept_countersigning_request");

    Ok(PreflightRequestAcceptance::Accepted(
        PreflightResponse::try_new(request, countersigning_agent_state, signature)?,
    ))
}
