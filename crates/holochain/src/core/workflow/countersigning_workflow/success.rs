use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::workflow::countersigning_workflow::CountersigningSessionState;
use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::SignedAction;

/// An incoming countersigning session success.
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(space, signed_actions, countersigning_trigger))
)]
pub(crate) async fn countersigning_success(
    space: Space,
    author: AgentPubKey,
    signature_bundle: Vec<SignedAction>,
    countersigning_trigger: TriggerSender,
) {
    let should_trigger = space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            match inner.sessions.entry(author.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    match entry.get() {
                        // If we're in the accepted state, then this is the happy path.
                        // Switch to the signatures collected state.
                        CountersigningSessionState::Accepted(preflight_request) => {
                            tracing::trace!("Received countersigning signature bundle in the accepted state for agent: {:?}", author);
                            entry.insert(CountersigningSessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: vec![signature_bundle],
                                resolution: None,
                            });
                        }
                        // This could happen but is relatively unlikely. If we've restarted and gone
                        // into the unknown state, then receive valid signatures, we may as well
                        // use them. So we'll switch to the signatures collected state.
                        CountersigningSessionState::Unknown { preflight_request, resolution } => {
                            tracing::trace!("Received countersigning signature bundle in the unknown state for agent: {:?}", author);
                            entry.insert(CountersigningSessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: vec![signature_bundle],
                                // We must guarantee that this value is always `Some` before switching
                                // to signatures collected so that signatures collected knows we
                                // transitioned from this state.
                                resolution: Some(resolution.clone().unwrap_or_default()),
                            });
                        }
                        CountersigningSessionState::SignaturesCollected { preflight_request, signature_bundles, resolution} => {
                            tracing::trace!("Received another signature bundle for countersigning session for agent: {:?}", author);
                            entry.insert(CountersigningSessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: {
                                    let mut signature_bundles = signature_bundles.clone();
                                    signature_bundles.push(signature_bundle);
                                    signature_bundles
                                },
                                resolution: resolution.clone(),
                            });
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
