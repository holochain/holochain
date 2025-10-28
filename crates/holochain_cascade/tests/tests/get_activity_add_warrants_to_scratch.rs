use ::fixt::fixt;
use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator};
use holo_hash::AgentPubKey;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_zome_types::fixt::SignatureFixturator;
use std::sync::Arc;

// Test that fetching agent activity from the network adds received warrants
// to the scratch.
//
// Only warrants coming from the network should be added to the scratch. If
// the caller is an authority for the agent, warrants are already locally present
// and shouldn't be redundantly added.
#[tokio::test(flavor = "multi_thread")]
async fn get_as_non_authority_adds_warrants_to_scratch() {
    holochain_trace::test_run();

    let agent_key = fixt!(AgentPubKey);

    // Create a test warrant
    let signed_warrant = create_test_warrant(&agent_key);

    // Create network and cascade
    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_authority_for_hash()
        // Not the authority for the agent, so that the call goes to the network.
        .returning(|_hash| Ok(false));
    network.expect_get_agent_activity().return_once({
        let warrant = signed_warrant.clone();
        move |agent, _filter, _options| {
            let agent_activity_response = AgentActivityResponse {
                agent,
                highest_observed: None,
                rejected_activity: ChainItems::NotRequested,
                valid_activity: ChainItems::NotRequested,
                status: ChainStatus::Empty,
                warrants: vec![warrant],
            };
            Ok(vec![agent_activity_response])
        }
    });
    let TestCase { scratch, cascade } = TestCase::new(network);

    // Call get_agent_activity - this should return warrants and add them to scratch
    let response = cascade
        .get_agent_activity(
            agent_key.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions::default(),
        )
        .await
        .unwrap();

    // Verify that the response contains the warrant
    assert_eq!(response.warrants.len(), 1);
    assert_eq!(response.warrants[0], signed_warrant);

    // Verify the scratch contains the warrant too.
    let warrants_in_scratch = scratch
        .apply_and_then(|scratch| {
            SourceChainResult::Ok(scratch.warrants().cloned().collect::<Vec<_>>())
        })
        .unwrap();
    assert_eq!(warrants_in_scratch.len(), 1);
    assert_eq!(warrants_in_scratch[0], signed_warrant);
}

// Test that fetching agent activity locally does not add received warrants
// to the scratch.
#[tokio::test(flavor = "multi_thread")]
async fn local_get_as_authority_does_not_add_warrants_to_scratch() {
    holochain_trace::test_run();

    // Create network and cascade
    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_authority_for_hash()
        // Authority for the agent, so call should be local.
        .returning(|_hash| Ok(true));
    let TestCase { scratch, cascade } = TestCase::new(network);

    // DHT is needed to query for warrants locally.
    let dht = test_dht_db();

    let agent_key = fixt!(AgentPubKey);

    // Create a test warrant
    let signed_warrant = create_test_warrant(&agent_key);
    let warrant_op =
        DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(signed_warrant.clone().into())));

    // Insert warrant into DHT database for local querying.
    dht.test_write(move |txn| {
        insert_op_dht(txn, &warrant_op, 0, None).unwrap();
        set_validation_status(txn, &warrant_op.hash, ValidationStatus::Valid).unwrap();
        set_when_integrated(txn, &warrant_op.hash, Timestamp::now()).unwrap();
    });

    // Add DHT db to cascade
    let cascade = cascade.with_dht(dht.to_db().into());

    // Calling get_agent_activity should not add warrants to scratch.
    let response = cascade
        .get_agent_activity(
            agent_key.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                get_options: GetOptions::local(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // Verify that the response contains the warrant.
    assert_eq!(response.warrants.len(), 1);
    assert_eq!(response.warrants[0], signed_warrant);

    // Verify the scratch does not contain warrants.
    let warrants_in_scratch = scratch
        .apply_and_then(|scratch| {
            SourceChainResult::Ok(scratch.warrants().cloned().collect::<Vec<_>>())
        })
        .unwrap();
    assert_eq!(warrants_in_scratch.len(), 0);
}

struct TestCase {
    scratch: SyncScratch,
    cascade: CascadeImpl,
}

impl TestCase {
    fn new(network: MockHolochainP2pDnaT) -> Self {
        let cache = test_cache_db();
        // Create a scratch to test warrant addition
        let scratch = Scratch::new().into_sync();

        let network = Arc::new(network);
        let cascade = CascadeImpl::empty()
            .with_network(network, cache.to_db())
            .with_scratch(scratch.clone());
        TestCase { scratch, cascade }
    }
}

fn create_test_warrant(agent_key: &AgentPubKey) -> SignedWarrant {
    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: agent_key.clone(),
            action: (fixt!(ActionHash), fixt!(Signature)),
            chain_op_type: ChainOpType::StoreEntry,
        }),
        fixt!(AgentPubKey),
        Timestamp::now(),
        agent_key.clone(),
    );
    SignedWarrant::new(warrant, fixt!(Signature))
}
