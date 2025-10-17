use ::fixt::fixt;
use holo_hash::{
    fixt::{ActionHashFixturator, DnaHashFixturator},
    HashableContentExtSync,
};
use holochain_cascade::{authority, CascadeImpl};
use holochain_keystore::test_keystore;
use holochain_p2p::{
    event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig, HolochainP2pDna,
};
use holochain_state::prelude::{
    insert_op_dht, set_validation_status, test_cache_db, test_conductor_db, test_dht_db,
};
use holochain_types::{
    db::{DbKindDht, DbKindPeerMetaStore, DbWrite},
    dht_op::{ChainOp, DhtOp, DhtOpHashed},
    fixt::{ActionFixturator, CreateFixturator, EntryFixturator, SignatureFixturator},
    prelude::{Judged, SignedAction, ValidationStatus},
};
use holochain_zome_types::{op::ChainOpType, Action};
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread")]
async fn t() {
    holochain_trace::test_run();
    let dna_hash = fixt!(DnaHash);
    let alice_keystore = test_keystore();
    let alice_dht = test_dht_db();
    let alice_cache = test_cache_db();
    let alice_op_db = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
    let alice_peer_meta_db =
        DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
    let alice_network = spawn_holochain_p2p(
        HolochainP2pConfig {
            get_conductor_db: Arc::new(|| Box::pin(async { test_conductor_db().to_db() })),
            get_db_op_store: Arc::new(move |_| {
                let op_db = alice_op_db.clone();
                Box::pin(async move { Ok(op_db) })
            }),
            get_db_peer_meta: Arc::new(move |_| {
                let peer_meta_db = alice_peer_meta_db.clone();
                Box::pin(async move { Ok(peer_meta_db) })
            }),
            k2_test_builder: true,
            ..Default::default()
        },
        alice_keystore.clone(),
    )
    .await
    .unwrap();
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
    alice_network
        .register_handler(Arc::new(alice_handler))
        .await
        .unwrap();
    let alice = alice_keystore.new_sign_keypair_random().await.unwrap();
    alice_network
        .join(dna_hash.clone(), alice.clone(), None)
        .await
        .unwrap();
    // Alice should respond to all get requests.
    alice_network
        .test_set_full_arcs(dna_hash.to_k2_space())
        .await;
    let alice_network = Arc::new(HolochainP2pDna::new(alice_network, dna_hash.clone(), None));
    let alice_cascade = CascadeImpl::empty()
        .with_dht(alice_dht.to_db().into())
        .with_network(alice_network, alice_cache.to_db());

    let bob_keystore = test_keystore();
    let bob_cache = test_cache_db();
    let bob_op_db = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
    let bob_peer_meta_db =
        DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
    let bob_network = spawn_holochain_p2p(
        HolochainP2pConfig {
            get_conductor_db: Arc::new(|| Box::pin(async { test_conductor_db().to_db() })),
            get_db_op_store: Arc::new(move |_| {
                let op_db = bob_op_db.clone();
                Box::pin(async move { Ok(op_db) })
            }),
            get_db_peer_meta: Arc::new(move |_| {
                let peer_meta_db = bob_peer_meta_db.clone();
                Box::pin(async move { Ok(peer_meta_db) })
            }),
            k2_test_builder: true,
            ..Default::default()
        },
        bob_keystore.clone(),
    )
    .await
    .unwrap();
    bob_network
        .register_handler(Arc::new(MockHcP2pHandler::new()))
        .await
        .unwrap();
    let bob = bob_keystore.new_sign_keypair_random().await.unwrap();
    bob_network
        .join(dna_hash.clone(), bob.clone(), None)
        .await
        .unwrap();
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
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let bob_network = Arc::new(HolochainP2pDna::new(bob_network, dna_hash.clone(), None));
    let bob_cascade = CascadeImpl::empty()
        .with_cache(bob_cache.to_db())
        .with_network(bob_network, bob_cache.to_db());

    let action_hash = fixt!(ActionHash);
    let maybe_chain_op = bob_cascade
        .fetch_op_by_type(action_hash, ChainOpType::RegisterAgentActivity)
        .await
        .unwrap();
    assert_eq!(maybe_chain_op, None);

    let mut create = fixt!(Create);
    let entry = fixt!(Entry);
    create.entry_hash = entry.to_hash();
    let action = SignedAction::new(Action::Create(create), fixt!(Signature));
    let action_hash = action.to_hash();
    let chain_op =
        ChainOp::from_type(ChainOpType::RegisterAgentActivity, action, Some(entry)).unwrap();
    // let validation_status = ValidationStatus::Valid;

    // Insert op into Alice's DHT db.
    alice_dht.test_write({
        let chain_op = chain_op.clone();
        move |txn| {
            insert_op_dht(
                txn,
                &DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op))),
                0,
                None,
            )
            .unwrap();
            set_validation_status(txn, hash, status)
        }
    });

    let maybe_chain_op = bob_cascade
        .fetch_op_by_type(action_hash, ChainOpType::RegisterAgentActivity)
        .await
        .unwrap();
    assert_eq!(
        maybe_chain_op,
        Some(Judged::new(chain_op, ValidationStatus::Valid))
    );
}
