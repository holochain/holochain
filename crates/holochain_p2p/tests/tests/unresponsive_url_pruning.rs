use holo_hash::DnaHash;
use holochain_data::kind::PeerMetaStore as PeerMetaStoreKind;
use holochain_keystore::{test_keystore, MetaLairClient};
use holochain_p2p::{
    actor::DynHcP2p, event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig,
    HolochainP2pLocalAgent,
};
use holochain_state::prelude::test_db_dir;
use holochain_types::db::{DbKindCache, DbKindDht, DbWrite};
use kitsune2_api::{
    AgentInfo, AgentInfoSigned, DhtArc, DynPeerMetaStore, SpaceId, Timestamp, Url, KEY_PREFIX_ROOT,
    META_KEY_UNRESPONSIVE,
};
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread")]
async fn urls_are_pruned_at_an_interval() {
    holochain_trace::test_run();
    let TestCase {
        peer_meta_store,
        db_peer_meta,
        _dir,
        ..
    } = TestCase::spawn().await;

    // Insert an unresponsive URL into peer meta store.
    // Expiry time needs to be more than 1 second in the future as we might be on the boundary of
    // this second.
    let unresponsive_peer = Url::from_str("ws://nowhere.land:80").unwrap();
    let expiry = Timestamp::now() + Duration::from_secs(2);
    let when = Timestamp::now();
    peer_meta_store
        .set_unresponsive(unresponsive_peer.clone(), expiry, when)
        .await
        .unwrap();

    // Wait for pruning to happen but check before the expiry, to make sure it has not expired yet.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let when_peer_set_unresponsive = peer_meta_store
        .get_unresponsive(unresponsive_peer.clone())
        .await
        .unwrap();
    assert_eq!(when_peer_set_unresponsive, Some(when));

    let unresponsive_urls = count_rows_in_peer_meta_store(&db_peer_meta).await;
    assert_eq!(unresponsive_urls, 1);

    // Wait until the expiry time or after.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check the row has actually been deleted from the table.
    let unresponsive_urls = count_rows_in_peer_meta_store(&db_peer_meta).await;
    assert_eq!(unresponsive_urls, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn urls_are_pruned_when_updated_agent_info_available() {
    holochain_trace::test_run();
    let TestCase {
        p2p,
        space_id,
        lair_client,
        peer_meta_store,
        db_peer_meta,
        _dir,
    } = TestCase::spawn().await;

    let earliest_timestamp = Timestamp::from_micros(Timestamp::now().as_micros() - 10_000_000); // 10s ago

    // Insert Alices's URL into peer meta store, as unresponsive.
    let alice_url = Url::from_str("ws://alice:80").unwrap();
    let expiry = Timestamp::from_micros(Timestamp::now().as_micros() + 10_000_000); // 10 s from now
    peer_meta_store
        .set_unresponsive(alice_url.clone(), expiry, Timestamp::now())
        .await
        .unwrap();
    assert!(peer_meta_store
        .get_unresponsive(alice_url.clone())
        .await
        .unwrap()
        .is_some());

    // Insert Bob's URL into peer meta store, as unresponsive.
    let bob_url = Url::from_str("ws://bob:80").unwrap();
    peer_meta_store
        .set_unresponsive(bob_url.clone(), expiry, Timestamp::now())
        .await
        .unwrap();
    assert!(peer_meta_store
        .get_unresponsive(bob_url.clone())
        .await
        .unwrap()
        .is_some());

    // Insert Alice's initial AgentInfo into peer store.
    // This AgentInfo was created **before** Alice's peer meta store unresponsive entry,
    // so it should **not** cause the peer meta store entry to be pruned.
    let alice_pubkey = lair_client.new_sign_keypair_random().await.unwrap();
    let alice_agent =
        HolochainP2pLocalAgent::new(alice_pubkey.clone(), DhtArc::FULL, 1, lair_client.clone());
    let alice_initial_agent_info = AgentInfoSigned::sign(
        &alice_agent,
        AgentInfo {
            agent: alice_pubkey.to_k2_agent(),
            created_at: earliest_timestamp,
            expires_at: Timestamp::from_micros(Timestamp::now().as_micros() + 10_000_000),
            is_tombstone: false,
            space: space_id.clone(),
            storage_arc: DhtArc::FULL,
            url: Some(alice_url.clone()),
        },
    )
    .await
    .unwrap();
    p2p.test_kitsune()
        .space_if_exists(space_id.clone())
        .await
        .unwrap()
        .peer_store()
        .insert(vec![alice_initial_agent_info])
        .await
        .unwrap();

    // Wait for at least one pruning.
    // To confirm pruning has completed, we add a third entry that is already expired, and then loop until it has been pruned.
    peer_meta_store
        .set_unresponsive(
            Url::from_str("ws://carol:80").unwrap(),
            earliest_timestamp,
            earliest_timestamp,
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let peer_meta_store_count = count_rows_in_peer_meta_store(&db_peer_meta).await;
            if peer_meta_store_count == 2 {
                break;
            }
        }
    })
    .await
    .unwrap();

    // Alice's URL should still be in the peer meta store.
    assert!(peer_meta_store
        .get_unresponsive(alice_url.clone())
        .await
        .unwrap()
        .is_some());

    // Alice and Bob's URLs should still be in the peer meta store.
    let peer_meta_store_count = count_rows_in_peer_meta_store(&db_peer_meta).await;
    assert_eq!(peer_meta_store_count, 2);

    // Insert Alice's updated AgentInfo into peer store.
    // This AgentInfo was created **after** Alice's peer meta store unresponsive entry,
    // so it should cause the peer meta store entry to be pruned.
    let alice_updated_agent_info = AgentInfoSigned::sign(
        &alice_agent,
        AgentInfo {
            agent: alice_pubkey.to_k2_agent(),
            created_at: Timestamp::now(),
            expires_at: Timestamp::from_micros(Timestamp::now().as_micros() + 10_000_000),
            is_tombstone: false,
            space: space_id.clone(),
            storage_arc: DhtArc::FULL,
            url: Some(alice_url.clone()),
        },
    )
    .await
    .unwrap();
    p2p.test_kitsune()
        .space_if_exists(space_id)
        .await
        .unwrap()
        .peer_store()
        .insert(vec![alice_updated_agent_info])
        .await
        .unwrap();

    // Wait for at least one pruning.
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let maybe_alice_entry = peer_meta_store
                .get_unresponsive(alice_url.clone())
                .await
                .unwrap();

            // Alice's URL should have been removed from the store
            // and Bob's URL should still be in the store.
            let peer_meta_store_count = count_rows_in_peer_meta_store(&db_peer_meta).await;
            if maybe_alice_entry.is_none() && peer_meta_store_count == 1 {
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn new_agent_with_same_url_prunes_old_unresponsive_entry() {
    holochain_trace::test_run();
    let TestCase {
        p2p,
        space_id,
        lair_client,
        peer_meta_store,
        db_peer_meta,
        _dir,
    } = TestCase::spawn().await;

    let max_expiry = Timestamp::from_micros(i64::MAX);
    let shared_url = Url::from_str("ws://shared:80").unwrap();

    // Mark the shared URL as unresponsive now.
    let unresponsive_at = Timestamp::now();
    peer_meta_store
        .set_unresponsive(shared_url.clone(), max_expiry, unresponsive_at)
        .await
        .unwrap();

    // Agent 1 has `created_at` before the unresponsive timestamp.
    let pubkey1 = lair_client.new_sign_keypair_random().await.unwrap();
    let agent1 = HolochainP2pLocalAgent::new(pubkey1.clone(), DhtArc::FULL, 1, lair_client.clone());
    let agent1_info = AgentInfoSigned::sign(
        &agent1,
        AgentInfo {
            agent: pubkey1.to_k2_agent(),
            created_at: (unresponsive_at - Duration::from_secs(1)).unwrap(),
            expires_at: max_expiry,
            is_tombstone: false,
            space: space_id.clone(),
            storage_arc: DhtArc::FULL,
            url: Some(shared_url.clone()),
        },
    )
    .await
    .unwrap();

    // Agent 2 has `created_at` after the unresponsive timestamp.
    let pubkey2 = lair_client.new_sign_keypair_random().await.unwrap();
    let agent2 = HolochainP2pLocalAgent::new(pubkey2.clone(), DhtArc::FULL, 1, lair_client.clone());
    let agent2_info = AgentInfoSigned::sign(
        &agent2,
        AgentInfo {
            agent: pubkey2.to_k2_agent(),
            created_at: unresponsive_at + Duration::from_secs(1),
            expires_at: max_expiry,
            is_tombstone: false,
            space: space_id.clone(),
            storage_arc: DhtArc::FULL,
            url: Some(shared_url.clone()),
        },
    )
    .await
    .unwrap();

    // Insert the older agent 1.
    p2p.test_kitsune()
        .space_if_exists(space_id.clone())
        .await
        .unwrap()
        .peer_store()
        .insert(vec![agent1_info])
        .await
        .unwrap();

    // Check that the URL is marked as unresponsive.
    let unresponsive_urls = count_rows_in_peer_meta_store(&db_peer_meta).await;
    assert_eq!(unresponsive_urls, 1);

    // Insert the new agent 2 that was created after the unresponsive_at time.
    p2p.test_kitsune()
        .space_if_exists(space_id.clone())
        .await
        .unwrap()
        .peer_store()
        .insert(vec![agent2_info])
        .await
        .unwrap();

    // Allow the pruning to happen after a new agent is added with the previously unresponsive URL.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check that the URL is marked as unresponsive.
    let unresponsive_urls = count_rows_in_peer_meta_store(&db_peer_meta).await;
    assert_eq!(unresponsive_urls, 0);
}

struct TestCase {
    p2p: DynHcP2p,
    space_id: SpaceId,
    lair_client: MetaLairClient,
    peer_meta_store: DynPeerMetaStore,
    db_peer_meta: holochain_data::DbWrite<PeerMetaStoreKind>,
    _dir: tempfile::TempDir,
}

impl TestCase {
    pub async fn spawn() -> Self {
        let dna_hash = DnaHash::from_raw_32(vec![0xaa; 32]);
        let space_id = dna_hash.to_k2_space();
        // Using a temp file for the peer meta DB, because the in memory one uses shared cache
        // which can't be accessed from two connections at the same time.

        // The DB logic expects the folder to have a parent folder.
        let dir = test_db_dir();
        let db_dir = dir.path().join("tmp_database");
        std::fs::create_dir(&db_dir).unwrap();
        let db_peer_meta = holochain_data::open_db(
            &db_dir,
            PeerMetaStoreKind::new(Arc::new(dna_hash.clone())),
            holochain_data::HolochainDataConfig::default(),
        )
        .await
        .unwrap();
        let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
        let db_cache = DbWrite::test_in_mem(DbKindCache(Arc::new(dna_hash.clone()))).unwrap();
        let conductor_store = holochain_state::conductor::ConductorStore::new_test()
            .await
            .unwrap();
        let lair_client = test_keystore();
        let db_peer_meta2 = db_peer_meta.clone();
        let p2p = spawn_holochain_p2p(
            HolochainP2pConfig {
                get_db_peer_meta: Arc::new(move |_| {
                    let db_peer_meta2 = db_peer_meta2.clone();
                    Box::pin(async move { Ok(db_peer_meta2.clone()) })
                }),
                peer_meta_pruning_interval_ms: 1,
                get_db_op_store: Arc::new(move |_| {
                    let db_op = db_op.clone();
                    Box::pin(async move { Ok(db_op) })
                }),
                get_db_cache: Arc::new(move |_| {
                    let db_cache = db_cache.clone();
                    Box::pin(async move { Ok(db_cache) })
                }),
                get_conductor_store: Arc::new(move || {
                    let conductor_store = conductor_store.clone();
                    Box::pin(async move { conductor_store })
                }),
                network_config: Some(serde_json::json!({
                    "coreBootstrap": {
                        "serverUrl": "https://not-used"
                    },
                    "tx5Transport": {
                        "serverUrl": "wss://not-used",
                        "timeoutS": 30,
                        "webrtcConnectTimeoutS": 25,
                    }
                })),
                ..Default::default()
            },
            lair_client.clone(),
        )
        .await
        .unwrap();
        p2p.register_handler(Arc::new(MockHcP2pHandler::new()))
            .await
            .unwrap();
        let peer_meta_store = p2p
            .test_kitsune()
            .space(space_id.clone(), None)
            .await
            .unwrap()
            .peer_meta_store()
            .clone();

        Self {
            p2p,
            space_id,
            lair_client,
            peer_meta_store,
            db_peer_meta,
            _dir: dir,
        }
    }
}

async fn count_rows_in_peer_meta_store(db: &holochain_data::DbWrite<PeerMetaStoreKind>) -> usize {
    let key = format!("{KEY_PREFIX_ROOT}:{META_KEY_UNRESPONSIVE}");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM peer_meta WHERE meta_key = ?")
        .bind(&key)
        .fetch_one(db.pool())
        .await
        .unwrap();
    count as usize
}
