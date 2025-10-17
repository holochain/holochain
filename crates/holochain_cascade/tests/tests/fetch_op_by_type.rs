use ::fixt::fixt;
use holo_hash::{fixt::DnaHashFixturator, AgentPubKey, DnaHash, HashableContentExtSync};
use holochain_cascade::{authority, test_utils::create_test_chain_op, CascadeImpl};
use holochain_keystore::test_keystore;
use holochain_p2p::{
    actor::DynHcP2p, event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig,
    HolochainP2pDna,
};
use holochain_state::prelude::{
    insert_op_dht, set_validation_status, set_when_integrated, test_cache_db, test_conductor_db,
    test_dht_db,
};
use holochain_types::{
    db::{DbKindDht, DbKindPeerMetaStore, DbWrite},
    dht_op::{DhtOp, DhtOpHashed},
    prelude::{Judged, Timestamp, ValidationStatus},
};
use holochain_zome_types::op::ChainOpType;
use rand::seq::IteratorRandom;
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread")]
async fn fetch_op_by_type_over_network() {
    holochain_trace::test_run();
    let dna_hash = fixt!(DnaHash);

    // Set up Alice as the serving node.
    let alice_dht = test_dht_db();
    let mut alice_handler = MockHcP2pHandler::new();
    let alice_dht_2 = alice_dht.clone();
    alice_handler
        .expect_handle_get_by_op_type()
        .returning(move |_, _, action_hash, op_type| {
            let alice_dht = alice_dht_2.clone();
            Box::pin(async move {
                Ok(
                    authority::handle_get_by_op_type(alice_dht.into(), action_hash, op_type)
                        .await
                        .unwrap(),
                )
            })
        });
    let TestCase {
        network: alice_network,
        agent: alice,
    } = TestCase::new(&dna_hash, alice_handler).await;

    // Alice should respond to all get requests.
    alice_network
        .test_set_full_arcs(dna_hash.to_k2_space())
        .await;

    // Set up Bob as the requesting node.
    let bob_cache = test_cache_db();
    let TestCase {
        network: bob_network,
        ..
    } = TestCase::new(&dna_hash, MockHcP2pHandler::new()).await;

    // Wait for Bob to see Alice in his peer store.
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if bob_network
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get(alice.to_k2_agent())
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Set up Bob's cascade.
    let bob_network = Arc::new(HolochainP2pDna::new(bob_network, dna_hash.clone(), None));
    let bob_cascade = CascadeImpl::empty()
        .with_cache(bob_cache.to_db())
        .with_network(bob_network, bob_cache.to_db());

    // Test all ChainOpType variants
    let op_types = vec![
        ChainOpType::StoreRecord,
        ChainOpType::StoreEntry,
        ChainOpType::RegisterAgentActivity,
        ChainOpType::RegisterUpdatedContent,
        ChainOpType::RegisterUpdatedRecord,
        ChainOpType::RegisterDeletedBy,
        ChainOpType::RegisterDeletedEntryAction,
        ChainOpType::RegisterAddLink,
        ChainOpType::RegisterRemoveLink,
    ];

    for op_type in op_types {
        let chain_op = create_test_chain_op(op_type);
        let action_hash = chain_op.action().to_hash();

        // Use a random validation status for each op type.
        let validation_statuses = vec![
            ValidationStatus::Abandoned,
            ValidationStatus::Rejected,
            ValidationStatus::Valid,
        ];
        let validation_status = validation_statuses
            .iter()
            .choose(&mut rand::rng())
            .unwrap()
            .clone();

        let maybe_chain_op = bob_cascade
            .fetch_op_by_type(action_hash.clone(), chain_op.get_type())
            .await
            .unwrap();
        assert_eq!(maybe_chain_op, None);

        // Insert op into Alice's DHT db.
        alice_dht.test_write({
            let chain_op = chain_op.clone();
            move |txn| {
                let dht_op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)));
                insert_op_dht(txn, &dht_op, 0, None).unwrap();
                set_validation_status(txn, &dht_op.hash, validation_status).unwrap();
                set_when_integrated(txn, &dht_op.hash, Timestamp::now()).unwrap();
            }
        });

        let maybe_chain_op = bob_cascade
            .fetch_op_by_type(action_hash, chain_op.get_type())
            .await
            .unwrap();
        assert_eq!(
            maybe_chain_op,
            Some(Judged::new(chain_op, validation_status))
        );
    }
}

struct TestCase {
    network: DynHcP2p,
    agent: AgentPubKey,
}

impl TestCase {
    async fn new(dna_hash: &DnaHash, p2p_handler: MockHcP2pHandler) -> Self {
        let keystore = test_keystore();
        let op_db = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
        let peer_meta_db =
            DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
        let network = spawn_holochain_p2p(
            HolochainP2pConfig {
                get_conductor_db: Arc::new(|| Box::pin(async { test_conductor_db().to_db() })),
                get_db_op_store: Arc::new(move |_| {
                    let op_db = op_db.clone();
                    Box::pin(async move { Ok(op_db) })
                }),
                get_db_peer_meta: Arc::new(move |_| {
                    let peer_meta_db = peer_meta_db.clone();
                    Box::pin(async move { Ok(peer_meta_db) })
                }),
                k2_test_builder: true,
                ..Default::default()
            },
            keystore.clone(),
        )
        .await
        .unwrap();
        network
            .register_handler(Arc::new(p2p_handler))
            .await
            .unwrap();
        let agent = keystore.new_sign_keypair_random().await.unwrap();
        network
            .join(dna_hash.clone(), agent.clone(), None)
            .await
            .unwrap();
        Self { network, agent }
    }
}
