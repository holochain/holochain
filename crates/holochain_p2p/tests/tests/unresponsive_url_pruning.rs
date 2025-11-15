use holo_hash::DnaHash;
use holochain_keystore::{test_keystore, MetaLairClient};
use holochain_p2p::{
    actor::DynHcP2p, event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig,
    HolochainP2pLocalAgent,
};
use holochain_state::prelude::{named_params, test_db_dir};
use holochain_types::db::{DbKindConductor, DbKindDht, DbKindPeerMetaStore, DbWrite};
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
        ..
    } = TestCase::spawn().await;

    // Insert an unresponsive URL into peer meta store.
    let unresponsive_peer = Url::from_str("ws://nowhere.land:80").unwrap();
    let expiry = Timestamp::from_micros(Timestamp::now().as_micros() + 500_000); // 500 ms from now
    let when = Timestamp::now();
    peer_meta_store
        .set_unresponsive(unresponsive_peer.clone(), expiry, when)
        .await
        .unwrap();

    // Waiting until the next pruning, but before the expiry, to make sure expiry is respected.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let when_peer_set_unresponsive = peer_meta_store
        .get_unresponsive(unresponsive_peer.clone())
        .await
        .unwrap();
    assert_eq!(when_peer_set_unresponsive, Some(when));

    let unresponsive_urls = count_rows_in_peer_meta_store(db_peer_meta.clone());
    assert_eq!(unresponsive_urls, 1);

    // Waiting until the next pruning, after expiry.
    // Test has to wait at least until the next second, because the expiry is compared with the unixepoch function in SQLite which returns
    // the timestamp in full seconds, plus the pruning interval.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            // This query filters expired rows.
            let when_peer_marked_unresponsive = peer_meta_store
                .get_unresponsive(unresponsive_peer.clone())
                .await
                .unwrap();
            // Check the row has actually been deleted from the table.
            let unresponsive_urls = count_rows_in_peer_meta_store(db_peer_meta.clone());
            if when_peer_marked_unresponsive.is_none() && unresponsive_urls == 0 {
                break;
            }
        }
    })
    .await
    .unwrap();
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
        .space(space_id.clone())
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

            let peer_meta_store_count = count_rows_in_peer_meta_store(db_peer_meta.clone());
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
    let peer_meta_store_count = count_rows_in_peer_meta_store(db_peer_meta.clone());
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
        .space(space_id)
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
            let peer_meta_store_count = count_rows_in_peer_meta_store(db_peer_meta.clone());
            if maybe_alice_entry.is_none() && peer_meta_store_count == 1 {
                break;
            }
        }
    })
    .await
    .unwrap();
}

struct TestCase {
    p2p: DynHcP2p,
    space_id: SpaceId,
    lair_client: MetaLairClient,
    peer_meta_store: DynPeerMetaStore,
    db_peer_meta: DbWrite<DbKindPeerMetaStore>,
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
        std::fs::create_dir(db_dir.clone()).unwrap();
        let db_peer_meta =
            DbWrite::test(&db_dir, DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
        let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
        let db_conductor = DbWrite::test_in_mem(DbKindConductor).unwrap();
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
                get_conductor_db: Arc::new(move || {
                    let db_conductor = db_conductor.clone();
                    Box::pin(async move { db_conductor })
                }),
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
            .space(space_id.clone())
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
        }
    }
}

fn count_rows_in_peer_meta_store(db: DbWrite<DbKindPeerMetaStore>) -> usize {
    db.test_read(|txn| {
        let mut stmt = txn
            .prepare("SELECT COUNT(*) FROM peer_meta WHERE meta_key = :meta_key")
            .unwrap();
        stmt.query_row(
            named_params! {":meta_key": format!("{KEY_PREFIX_ROOT}:{META_KEY_UNRESPONSIVE}")},
            |row| row.get::<_, usize>(0),
        )
        .unwrap()
    })
}
