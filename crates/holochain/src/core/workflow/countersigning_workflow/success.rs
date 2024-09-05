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
                        // Whether we're awaiting signatures for the first time or trying to recover,
                        // switch to the signatures collected state and add the signatures to the
                        // list of signature bundles to try.
                        // TODO never in unknown here
                        CountersigningSessionState::Accepted(ref preflight_request) | CountersigningSessionState::Unknown { ref preflight_request, .. } => {
                            tracing::trace!("Received countersigning signature bundle in accepted or unknown state for agent: {:?}", author);
                            entry.insert(CountersigningSessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: vec![signature_bundle],
                            });
                        }
                        CountersigningSessionState::SignaturesCollected { preflight_request, signature_bundles} => {
                            tracing::trace!("Received another signature bundle for countersigning session for agent: {:?}", author);
                            entry.insert(CountersigningSessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: {
                                    let mut signature_bundles = signature_bundles.clone();
                                    signature_bundles.push(signature_bundle);
                                    signature_bundles
                                },
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
