use bytes::Bytes;
use holo_hash::DnaHash;
use holochain_keystore::{test_keystore, MetaLairClient};
use holochain_p2p::{
    actor::DynHcP2p, event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig,
    HolochainP2pLocalAgent,
};
use holochain_state::prelude::{named_params, test_db_dir};
use holochain_types::db::{DbKindDht, DbKindPeerMetaStore, DbWrite};
use kitsune2_api::{
    AgentId, AgentInfo, AgentInfoSigned, DhtArc, DynPeerMetaStore, Id, SpaceId, Timestamp, Url,
    KEY_PREFIX_ROOT, META_KEY_UNRESPONSIVE,
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

    // Insert an unresponsive URL into peer meta store.
    let unresponsive_url = Url::from_str("ws://under.water:80").unwrap();
    let expiry = Timestamp::from_micros(Timestamp::now().as_micros() + 10_000_000); // 10 s from now
    peer_meta_store
        .set_unresponsive(unresponsive_url.clone(), expiry, Timestamp::now())
        .await
        .unwrap();
    peer_meta_store
        .set_unresponsive(unresponsive_url.clone(), expiry, Timestamp::now())
        .await
        .unwrap();
    // Mark another URL unresponsive, for which there won't be an updated agent info.
    let other_unresponsive_url = Url::from_str("ws://under.earth:80").unwrap();
    peer_meta_store
        .set_unresponsive(other_unresponsive_url.clone(), expiry, Timestamp::now())
        .await
        .unwrap();

    let maybe_unresponsive_url = peer_meta_store
        .get_unresponsive(unresponsive_url.clone())
        .await
        .unwrap();
    assert!(maybe_unresponsive_url.is_some());
    let maybe_other_unresponsive_url = peer_meta_store
        .get_unresponsive(other_unresponsive_url.clone())
        .await
        .unwrap();
    assert!(maybe_other_unresponsive_url.is_some());

    let agent = lair_client.new_sign_keypair_random().await.unwrap();
    let local_agent = HolochainP2pLocalAgent::new(agent.clone(), DhtArc::FULL, 1, lair_client);
    let updated_agent_info = AgentInfoSigned::sign(
        &local_agent,
        AgentInfo {
            agent: AgentId(Id(Bytes::from_static(b"a"))),
            created_at: Timestamp::now(),
            expires_at: Timestamp::from_micros(Timestamp::now().as_micros() + 10_000_000),
            is_tombstone: false,
            space: space_id.clone(),
            storage_arc: DhtArc::FULL,
            url: Some(unresponsive_url.clone()),
        },
    )
    .await
    .unwrap();
    p2p.test_kitsune()
        .space(space_id)
        .await
        .unwrap()
        .peer_store()
        .insert(vec![updated_agent_info])
        .await
        .unwrap();

    // Wait for at least one pruning.
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            // URL should have been removed from the store.
            let when_marked_unresponsive = peer_meta_store
                .get_unresponsive(unresponsive_url.clone())
                .await
                .unwrap();
            // Check the row has actually been deleted from the table. The other URL should still be present.
            let unresponsive_urls = count_rows_in_peer_meta_store(db_peer_meta.clone());
            if when_marked_unresponsive.is_none() && unresponsive_urls == 1 {
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
                k2_test_builder: true,
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
