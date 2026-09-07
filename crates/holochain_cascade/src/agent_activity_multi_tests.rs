//! Tests for the `get_agent_activity_multi` cascade passthrough.

use super::*;
use ::fixt::fixt;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_keystore::{test_keystore, AgentPubKeyExt};
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

/// A peer answering a multi call is not trusted to say what an agent authored.
/// Records it serves that are not signed by the action's author are dropped
/// before the caller sees them, even though the cascade otherwise passes each
/// peer's response through untouched.
// `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
#[tokio::test(flavor = "multi_thread")]
async fn agent_activity_multi_drops_records_not_signed_by_their_author() {
    let store = empty_store().await;
    let keystore = test_keystore();
    let author = holo_hash::AgentPubKey::new_random(&keystore).await.unwrap();
    let responder = holo_hash::AgentPubKey::new_random(&keystore).await.unwrap();

    let action = Action {
        header: ActionHeader {
            author: author.clone(),
            timestamp: Timestamp::from_micros(42),
            action_seq: 0,
            prev_action: None,
        },
        data: ActionData::Dna(DnaData {
            dna_hash: holo_hash::DnaHash::from_raw_36(vec![9u8; 36]),
        }),
    };
    let record = |signature| {
        Record::new(
            SignedActionHashed::with_presigned(
                holo_hash::HoloHashed::from_content_sync(action.clone()),
                signature,
            ),
            RecordEntry::or_not_applicable(None),
        )
    };
    let honest = record(author.sign(&keystore, &action).await.unwrap());
    let forged = record(responder.sign(&keystore, &action).await.unwrap());

    let canned = vec![(
        responder.clone(),
        AgentActivityResponse {
            agent: author.clone(),
            valid_activity: ChainItems::Full(vec![honest.clone(), forged]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            highest_observed: None,
            warrants: Vec::new(),
        },
    )];

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_get_agent_activity_multi()
        .times(1)
        .returning(move |_, _, _| Ok(canned.clone()));

    let cascade = CascadeImpl::empty(store).with_network(Arc::new(network));

    let responses = cascade
        .get_agent_activity_multi(
            author,
            ChainQueryFilter::new(),
            GetActivityMultiOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(responses.len(), 1);
    assert_eq!(
        responses[0].1.valid_activity,
        ChainItems::Full(vec![honest]),
        "the record the responder signed itself must not reach the caller"
    );
}
