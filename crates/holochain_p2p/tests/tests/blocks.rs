use crate::tests::common::spawn_test_bootstrap;
use ::fixt::fixt;
use holo_hash::{
    fixt::{ActionHashFixturator, AgentPubKeyFixturator, DhtOpHashFixturator, DnaHashFixturator},
    DnaHash,
};
use holochain_keystore::{test_keystore, MetaLairClient};
use holochain_p2p::{
    actor::DynHcP2p, event::MockHcP2pHandler, spawn_holochain_p2p, HolochainP2pConfig,
    HolochainP2pError, HolochainP2pLocalAgent,
};
use holochain_state::{block::get_all_cell_blocks, prelude::test_conductor_db};
use holochain_timestamp::{InclusiveTimestampInterval, Timestamp};
use holochain_types::{
    db::{DbKindConductor, DbKindDht, DbKindPeerMetaStore, DbWrite},
    prelude::{Block, BlockTargetId, CellBlockReason, CellId},
    record::WireRecordOps,
};
use holochain_zome_types::block::BlockTarget;
use kitsune2_api::{AgentInfo, AgentInfoSigned, DhtArc, DynBlocks};
use std::net::SocketAddr;
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread")]
async fn cell_blocks_are_committed_to_database() {
    let conductor_db = DbWrite::test_in_mem(DbKindConductor).unwrap();
    let dna_hash = fixt!(DnaHash);
    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let TestActor { actor, .. } =
        TestActor::new_with_conductor_db(&dna_hash, conductor_db.clone(), &addr).await;
    let cell_id = CellId::new(dna_hash, fixt!(AgentPubKey));
    let cell_block_reason = CellBlockReason::InvalidOp(fixt!(DhtOpHash));
    let block = Block::new(
        BlockTarget::Cell(cell_id.clone(), cell_block_reason.clone()),
        InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
    );

    actor.block(block).await.unwrap();

    let blocks = conductor_db.test_read(|txn| get_all_cell_blocks(txn));
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0].target(), BlockTarget::Cell(id, reason) if *id == cell_id && *reason == cell_block_reason)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_is_blocked() {
    let conductor_db = DbWrite::test_in_mem(DbKindConductor).unwrap();
    let dna_hash = fixt!(DnaHash);
    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let TestActor { actor, .. } =
        TestActor::new_with_conductor_db(&dna_hash, conductor_db.clone(), &addr).await;
    let agent = fixt!(AgentPubKey);
    let cell_id = CellId::new(dna_hash, agent.clone());
    let cell_block_reason = CellBlockReason::InvalidOp(fixt!(DhtOpHash));
    let block = Block::new(
        BlockTarget::Cell(cell_id.clone(), cell_block_reason.clone()),
        InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
    );

    let target = BlockTargetId::Cell(cell_id);
    assert!(!actor.is_blocked(target.clone()).await.unwrap());

    actor.block(block).await.unwrap();

    assert!(actor.is_blocked(target).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_is_removed_from_peer_store_when_blocked() {
    let dna_hash = fixt!(DnaHash);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let TestActor { actor: alice, .. } = TestActor::new(&dna_hash, &addr).await;
    let space = alice
        .test_kitsune()
        .space(dna_hash.to_k2_space(), None)
        .await
        .unwrap();
    let peer_store = space.peer_store();

    // Create Bob's keys so he can be blocked by Alice.
    let keystore = test_keystore();
    let bob_pubkey = keystore.new_sign_keypair_random().await.unwrap();
    let local_agent =
        HolochainP2pLocalAgent::new(bob_pubkey.clone(), DhtArc::FULL, 1, keystore.clone());
    let agent_info_signed = AgentInfoSigned::sign(
        &local_agent,
        AgentInfo {
            agent: bob_pubkey.to_k2_agent(),
            created_at: kitsune2_api::Timestamp::now(),
            expires_at: kitsune2_api::Timestamp::from_micros(i64::MAX),
            space: dna_hash.to_k2_space(),
            is_tombstone: false,
            storage_arc: DhtArc::Empty,
            url: None,
        },
    )
    .await
    .unwrap();
    peer_store.insert(vec![agent_info_signed]).await.unwrap();

    assert!(peer_store
        .get(bob_pubkey.to_k2_agent())
        .await
        .unwrap()
        .is_some());

    // Alice blocks Bob.
    let cell_id = CellId::new(dna_hash.clone(), bob_pubkey.clone());
    alice
        .block(Block::new(
            BlockTarget::Cell(cell_id, CellBlockReason::BadCrypto),
            InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
        ))
        .await
        .unwrap();

    // Check Bob has been removed from Alice's peer store.
    assert!(peer_store
        .get(bob_pubkey.to_k2_agent())
        .await
        .unwrap()
        .is_none());
}

mod blocks_impl {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn block_someone() {
        let dna_hash = fixt!(DnaHash);
        let agent = fixt!(AgentPubKey);
        let cell_id = CellId::new(dna_hash.clone(), agent.clone());

        let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
        let TestActor {
            actor,
            blocks_module,
        } = TestActor::new(&dna_hash, &addr).await;

        assert!(!blocks_module
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());
        assert!(!blocks_module.are_all_blocked(vec![]).await.unwrap());
        assert!(!blocks_module
            .are_all_blocked(vec![kitsune2_api::BlockTarget::Agent(agent.to_k2_agent())])
            .await
            .unwrap());

        // Block an agent.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id, CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Check agent is blocked now.
        assert!(blocks_module
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());
        assert!(blocks_module
            .are_all_blocked(vec![kitsune2_api::BlockTarget::Agent(agent.to_k2_agent())])
            .await
            .unwrap());
        // Empty target vector is still not blocked.
        assert!(!blocks_module.are_all_blocked(vec![]).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn are_all_blocked_mixed_then_all_blocked() {
        let dna_hash = fixt!(DnaHash);
        let agent_1 = fixt!(AgentPubKey);
        let agent_2 = fixt!(AgentPubKey);
        let cell_id_1 = CellId::new(dna_hash.clone(), agent_1.clone());
        let cell_id_2 = CellId::new(dna_hash.clone(), agent_2.clone());

        let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
        let TestActor {
            actor,
            blocks_module,
        } = TestActor::new(&dna_hash, &addr).await;

        // Initially not blocked.
        assert!(!blocks_module
            .are_all_blocked(vec![
                kitsune2_api::BlockTarget::Agent(agent_1.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent_2.to_k2_agent()),
            ])
            .await
            .unwrap());

        // Block agent1 only.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id_1, CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Mixed list should yield false.
        assert!(!blocks_module
            .are_all_blocked(vec![
                kitsune2_api::BlockTarget::Agent(agent_1.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent_2.to_k2_agent()),
            ])
            .await
            .unwrap());

        // Block agent2 as well.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id_2, CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Now all should be blocked.
        assert!(blocks_module
            .are_all_blocked(vec![
                kitsune2_api::BlockTarget::Agent(agent_1.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent_2.to_k2_agent()),
            ])
            .await
            .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn are_all_blocked_with_duplicate_targets() {
        let dna_hash = fixt!(DnaHash);
        let agent = fixt!(AgentPubKey);
        let cell_id = CellId::new(dna_hash.clone(), agent.clone());

        let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
        let TestActor {
            actor,
            blocks_module,
        } = TestActor::new(&dna_hash, &addr).await;

        // Not blocked initially even with duplicates in query.
        assert!(!blocks_module
            .are_all_blocked(vec![
                kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()),
            ])
            .await
            .unwrap());

        // Block the agent.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id, CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Duplicates in the query should still resolve to true once blocked.
        assert!(blocks_module
            .are_all_blocked(vec![
                kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()),
                kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()),
            ])
            .await
            .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn blocking_same_agent_twice_is_ok() {
        let dna_hash = fixt!(DnaHash);
        let agent = fixt!(AgentPubKey);
        let cell_id = CellId::new(dna_hash.clone(), agent.clone());

        let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
        let TestActor {
            actor,
            blocks_module,
        } = TestActor::new(&dna_hash, &addr).await;

        // First block.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id.clone(), CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Second block should not error.
        actor
            .block(Block::new(
                BlockTarget::Cell(cell_id, CellBlockReason::App(vec![])),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Still blocked.
        assert!(blocks_module
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());
    }

    // Same conductor DB, but two different DNA hashes.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_is_scoped_per_dna() {
        let agent = fixt!(AgentPubKey);
        let dna_hash_1 = fixt!(DnaHash);
        let dna_hash_2 = fixt!(DnaHash);
        let cell_id_1 = CellId::new(dna_hash_1.clone(), agent.clone());

        let conductor_db = test_conductor_db().to_db();
        let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
        let TestActor {
            actor: actor_1,
            blocks_module: blocks_module_1,
        } = TestActor::new_with_conductor_db(&dna_hash_1, conductor_db.clone(), &addr).await;
        let TestActor {
            blocks_module: blocks_module_2,
            ..
        } = TestActor::new_with_conductor_db(&dna_hash_2, conductor_db, &addr).await;

        // Initially not blocked in either DNA.
        assert!(!blocks_module_1
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());
        assert!(!blocks_module_2
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());

        // Block in DNA1 only.
        actor_1
            .block(Block::new(
                BlockTarget::Cell(cell_id_1, CellBlockReason::BadCrypto),
                InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()).unwrap(),
            ))
            .await
            .unwrap();

        // Blocked in DNA1.
        assert!(blocks_module_1
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());

        // Still not blocked in DNA2.
        assert!(!blocks_module_2
            .is_blocked(kitsune2_api::BlockTarget::Agent(agent.to_k2_agent()))
            .await
            .unwrap());

        // All-blocked checks respect DNA scoping too.
        assert!(blocks_module_1
            .are_all_blocked(vec![kitsune2_api::BlockTarget::Agent(agent.to_k2_agent())])
            .await
            .unwrap());
        assert!(!blocks_module_2
            .are_all_blocked(vec![kitsune2_api::BlockTarget::Agent(agent.to_k2_agent())])
            .await
            .unwrap());
    }
}

// Alice blocks Bob and makes a get request that fails.
#[tokio::test(flavor = "multi_thread")]
async fn get_to_blocked_agent_fails() {
    holochain_trace::test_run();
    let dna_hash = DnaHash::from_raw_32(vec![0xaa; 32]);
    let keystore_1 = test_keystore();
    let keystore_2 = test_keystore();
    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let TestActor { actor: alice, .. } =
        TestActor::new_with_keystore(&dna_hash, &keystore_1, &addr).await;
    let TestActor { actor: bob, .. } =
        TestActor::new_with_keystore(&dna_hash, &keystore_2, &addr).await;
    let alice_pubkey = keystore_1.new_sign_keypair_random().await.unwrap();
    let bob_pubkey = keystore_2.new_sign_keypair_random().await.unwrap();
    alice
        .join(dna_hash.clone(), alice_pubkey.clone(), None, None)
        .await
        .unwrap();
    bob.join(dna_hash.clone(), bob_pubkey.clone(), None, None)
        .await
        .unwrap();
    bob.test_set_full_arcs(dna_hash.to_k2_space()).await;

    // Exchange peer infos to accelerate bootstrapping.
    exchange_agent_infos(alice.clone(), bob.clone(), &dna_hash).await;

    // Before the block Alice can make get request and Bob answers them.
    let response = alice.get(dna_hash.clone(), fixt!(ActionHash).into()).await;
    assert!(
        response.is_ok(),
        "Expected get to succeed before block but got: {response:?}"
    );

    alice
        .block(Block::new(
            BlockTarget::Cell(
                CellId::new(dna_hash.clone(), bob_pubkey.clone()),
                CellBlockReason::BadCrypto,
            ),
            InclusiveTimestampInterval::try_new(
                holochain_timestamp::Timestamp::now(),
                holochain_timestamp::Timestamp::max(),
            )
            .unwrap(),
        ))
        .await
        .unwrap();

    // Blocking removes an agent from the peer store.
    // To make sure the block is being enforced, the agent should be re-added to the
    // peer 1's peer store. That isn't possible, however, because the peer store
    // implementation internally discards blocked agents when inserting.

    // Alice makes a get request. Bob is blocked, so there should be no one to respond.
    let response = alice.get(dna_hash.clone(), fixt!(ActionHash).into()).await;
    assert!(matches!(
        response,
        Err(HolochainP2pError::NoPeersForLocation(_, _))
    ));
}

// Alice blocks Bob and Bob makes a get request that fails.
#[tokio::test(flavor = "multi_thread")]
async fn get_by_blocked_agent_fails() {
    holochain_trace::test_run();
    let dna_hash = DnaHash::from_raw_32(vec![0xaa; 32]);
    let keystore_1 = test_keystore();
    let keystore_2 = test_keystore();
    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let TestActor { actor: alice, .. } =
        TestActor::new_with_keystore(&dna_hash, &keystore_1, &addr).await;
    let TestActor { actor: bob, .. } =
        TestActor::new_with_keystore(&dna_hash, &keystore_2, &addr).await;
    let alice_pubkey = keystore_1.new_sign_keypair_random().await.unwrap();
    let bob_pubkey = keystore_2.new_sign_keypair_random().await.unwrap();
    alice
        .join(dna_hash.clone(), alice_pubkey.clone(), None, None)
        .await
        .unwrap();
    bob.join(dna_hash.clone(), bob_pubkey.clone(), None, None)
        .await
        .unwrap();
    alice.test_set_full_arcs(dna_hash.to_k2_space()).await;

    // Exchange peer infos to accelerate bootstrapping.
    exchange_agent_infos(alice.clone(), bob.clone(), &dna_hash).await;

    // Before the block Bob can make get requests and Alice answers them.
    let response = bob.get(dna_hash.clone(), fixt!(ActionHash).into()).await;
    assert!(response.is_ok());

    alice
        .block(Block::new(
            BlockTarget::Cell(
                CellId::new(dna_hash.clone(), bob_pubkey.clone()),
                CellBlockReason::BadCrypto,
            ),
            InclusiveTimestampInterval::try_new(
                holochain_timestamp::Timestamp::now(),
                holochain_timestamp::Timestamp::max(),
            )
            .unwrap(),
        ))
        .await
        .unwrap();

    // Bob makes a get request. Alice could respond, but must not, to prove the block for incoming
    // requests is effective.
    let response = bob.get(dna_hash.clone(), fixt!(ActionHash).into()).await;
    assert!(response.is_err(), "expected error, got {response:?}");
}

struct TestActor {
    actor: DynHcP2p,
    blocks_module: DynBlocks,
}

impl TestActor {
    async fn new(dna_hash: &DnaHash, bootstrap_addr: &SocketAddr) -> Self {
        let conductor_db = DbWrite::test_in_mem(DbKindConductor).unwrap();
        Self::create_test_case(dna_hash, conductor_db, test_keystore(), bootstrap_addr).await
    }

    async fn new_with_conductor_db(
        dna_hash: &DnaHash,
        conductor_db: DbWrite<DbKindConductor>,
        bootstrap_addr: &SocketAddr,
    ) -> Self {
        Self::create_test_case(dna_hash, conductor_db, test_keystore(), bootstrap_addr).await
    }

    async fn new_with_keystore(
        dna_hash: &DnaHash,
        keystore: &MetaLairClient,
        bootstrap_addr: &SocketAddr,
    ) -> Self {
        let conductor_db = DbWrite::test_in_mem(DbKindConductor).unwrap();
        Self::create_test_case(dna_hash, conductor_db, keystore.clone(), bootstrap_addr).await
    }

    async fn create_test_case(
        dna_hash: &DnaHash,
        conductor_db: DbWrite<DbKindConductor>,
        keystore: MetaLairClient,
        bootstrap_addr: &SocketAddr,
    ) -> Self {
        let op_db = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
        let peer_meta_db =
            DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
        let config = HolochainP2pConfig {
            get_conductor_db: Arc::new(move || {
                let conductor_db = conductor_db.clone();
                Box::pin(async move { conductor_db })
            }),
            get_db_op_store: Arc::new(move |_| {
                let op_db = op_db.clone();
                Box::pin(async move { Ok(op_db) })
            }),
            get_db_peer_meta: Arc::new(move |_| {
                let peer_meta_db = peer_meta_db.clone();
                Box::pin(async move { Ok(peer_meta_db) })
            }),
            #[cfg(feature = "transport-tx5-backend-go-pion")]
            network_config: Some(serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": format!("http://{bootstrap_addr}"),
                },
                "tx5Transport": {
                    "serverUrl": format!("ws://{bootstrap_addr}"),
                    "signalAllowPlainText": true,
                    "timeoutS": 30,
                    "webrtcConnectTimeoutS": 25,
                }
            })),
            #[cfg(all(
                feature = "transport-iroh",
                not(feature = "transport-tx5-backend-go-pion")
            ))]
            network_config: Some(serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": format!("http://{bootstrap_addr}"),
                },
                "irohTransport": {
                    "relayUrl": format!("http://{bootstrap_addr}"),
                    "relayAllowPlainText": true,
                }
            })),
            request_timeout: Duration::from_secs(3),
            ..Default::default()
        };
        let actor = spawn_holochain_p2p(config, keystore).await.unwrap();
        let mut handler = MockHcP2pHandler::new();
        handler.expect_handle_get().returning(|_, _, _| {
            Box::pin(async move {
                Ok(holochain_types::dht_op::WireOps::Record(
                    WireRecordOps::new(),
                ))
            })
        });
        actor.register_handler(Arc::new(handler)).await.unwrap();
        let space = actor
            .test_kitsune()
            .space(dna_hash.to_k2_space(), None)
            .await
            .unwrap();
        let blocks_module = space.blocks().clone();
        Self {
            actor,
            blocks_module,
        }
    }
}

async fn exchange_agent_infos(alice: DynHcP2p, bob: DynHcP2p, dna_hash: &DnaHash) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let alice_agent_infos = alice
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap();
            let bob_agent_infos = bob
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap();
            if !alice_agent_infos.is_empty() && !bob_agent_infos.is_empty() {
                alice
                    .peer_store(dna_hash.clone())
                    .await
                    .unwrap()
                    .insert(bob_agent_infos)
                    .await
                    .unwrap();
                bob.peer_store(dna_hash.clone())
                    .await
                    .unwrap()
                    .insert(alice_agent_infos)
                    .await
                    .unwrap();
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("exchanging agent infos timed out");
}
