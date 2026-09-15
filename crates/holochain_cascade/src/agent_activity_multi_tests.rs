//! Tests for the `get_agent_activity_multi` cascade passthrough.

use super::*;
use ::fixt::fixt;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_p2p::actor::GetActivityMultiOptions;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_types::activity::AgentActivityResponse;

async fn empty_store() -> holochain_state::dht_store::DhtStore {
    let dna_hash = holo_hash::DnaHash::from_raw_36(vec![42u8; 36]);
    holochain_state::test_utils::test_dht_store(dna_hash).await
}

/// The cascade forwards the multi call to the network and returns the
/// per-peer responses untouched: no merging, no cache writes.
#[tokio::test]
async fn agent_activity_multi_passes_through_network_responses() {
    let store = empty_store().await;
    let queried_agent = fixt!(AgentPubKey);
    let responder = fixt!(AgentPubKey);

    let canned = vec![(
        responder.clone(),
        AgentActivityResponse {
            agent: queried_agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            highest_observed: None,
            warrants: Vec::new(),
        },
    )];

    let mut network = MockHolochainP2pDnaT::new();
    let mock_response = canned.clone();
    network
        .expect_get_agent_activity_multi()
        .times(1)
        .returning(move |_, _, _| Ok(mock_response.clone()));

    let cascade = CascadeImpl::empty(store).with_network(Arc::new(network));

    let responses = cascade
        .get_agent_activity_multi(
            queried_agent,
            ChainQueryFilter::new(),
            GetActivityMultiOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].0, responder);
}

/// A network `InsufficientResponses` (fewer than `required_responses`
/// peers answered with data in time) surfaces as an error instead of
/// being swallowed. This is the contract restore relies on: unlike
/// `fetch_agent_activity`, which downgrades some network failures to an
/// empty result, the multi call must never let a degraded network look
/// like a valid quorum sample.
#[tokio::test]
async fn agent_activity_multi_propagates_insufficient_responses_error() {
    let store = empty_store().await;

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_get_agent_activity_multi()
        .times(1)
        .returning(|_, _, _| {
            Err(holochain_p2p::HolochainP2pError::InsufficientResponses {
                operation: "get_agent_activity_multi".to_string(),
                received: 1,
                required: 2,
            })
        });

    let cascade = CascadeImpl::empty(store).with_network(Arc::new(network));

    let err = cascade
        .get_agent_activity_multi(
            fixt!(AgentPubKey),
            ChainQueryFilter::new(),
            GetActivityMultiOptions::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            CascadeError::NetworkError(holochain_p2p::HolochainP2pError::InsufficientResponses {
                received: 1,
                required: 2,
                ..
            })
        ),
        "unexpected error: {err:?}"
    );
}

/// Without a network handle the call fails loudly instead of returning an
/// empty vector — restore must never mistake "offline" for "no activity".
#[tokio::test]
async fn agent_activity_multi_without_network_errors() {
    let store = empty_store().await;
    let cascade = CascadeImpl::empty(store);

    let err = cascade
        .get_agent_activity_multi(
            fixt!(AgentPubKey),
            ChainQueryFilter::new(),
            GetActivityMultiOptions::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, CascadeError::NetworkNotInitialized));
}
