use ::fixt::fixt;
use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};
use holo_hash::{AgentPubKey, DnaHash};
use holochain_p2p::HolochainBlocks;
use holochain_state::block::block;
use holochain_state::prelude::{test_conductor_db, DbWrite};
use holochain_timestamp::{InclusiveTimestampInterval, Timestamp};
use holochain_types::db::DbKindConductor;
use holochain_types::prelude::{Block, CellBlockReason, CellId};
use kitsune2_api::{AgentId, BlockTarget, Blocks};

#[tokio::test(flavor = "multi_thread")]
async fn block_someone() {
    let dna_hash = fixt!(DnaHash);
    let agent = fixt!(AgentPubKey).to_k2_agent();
    let db = test_conductor_db().to_db();
    let blocks = HolochainBlocks::new(dna_hash.clone(), db.clone());
    assert!(!blocks
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());
    assert!(!blocks.are_all_blocked(vec![]).await.unwrap());
    assert!(!blocks
        .are_all_blocked(vec![BlockTarget::Agent(agent.clone())])
        .await
        .unwrap());

    // Block an agent.
    block_agent(&db, &dna_hash, &agent).await;

    // Check agent is blocked now.
    assert!(blocks
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());
    assert!(blocks
        .are_all_blocked(vec![BlockTarget::Agent(agent.clone())])
        .await
        .unwrap());
    // Empty target vector is still not blocked.
    assert!(!blocks.are_all_blocked(vec![]).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn are_all_blocked_mixed_then_all_blocked() {
    let dna_hash = fixt!(DnaHash);
    let agent1 = fixt!(AgentPubKey).to_k2_agent();
    let agent2 = fixt!(AgentPubKey).to_k2_agent();
    let db = test_conductor_db().to_db();
    let blocks = HolochainBlocks::new(dna_hash.clone(), db.clone());

    // Initially not blocked.
    assert!(!blocks
        .are_all_blocked(vec![
            BlockTarget::Agent(agent1.clone()),
            BlockTarget::Agent(agent2.clone()),
        ])
        .await
        .unwrap());

    // Block agent1 only.
    block_agent(&db, &dna_hash, &agent1).await;

    // Mixed list should yield false.
    assert!(!blocks
        .are_all_blocked(vec![
            BlockTarget::Agent(agent1.clone()),
            BlockTarget::Agent(agent2.clone()),
        ])
        .await
        .unwrap());

    // Block agent2 as well.
    block_agent(&db, &dna_hash, &agent2).await;

    // Now all should be blocked.
    assert!(blocks
        .are_all_blocked(vec![
            BlockTarget::Agent(agent1.clone()),
            BlockTarget::Agent(agent2.clone()),
        ])
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn are_all_blocked_with_duplicate_targets() {
    let dna_hash = fixt!(DnaHash);
    let agent = fixt!(AgentPubKey).to_k2_agent();
    let db = test_conductor_db().to_db();
    let blocks = HolochainBlocks::new(dna_hash.clone(), db.clone());

    // Not blocked initially even with duplicates in query.
    assert!(!blocks
        .are_all_blocked(vec![
            BlockTarget::Agent(agent.clone()),
            BlockTarget::Agent(agent.clone()),
            BlockTarget::Agent(agent.clone()),
        ])
        .await
        .unwrap());

    // Block the agent.
    block_agent(&db, &dna_hash, &agent).await;

    // Duplicates in the query should still resolve to true once blocked.
    assert!(blocks
        .are_all_blocked(vec![
            BlockTarget::Agent(agent.clone()),
            BlockTarget::Agent(agent.clone()),
        ])
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_same_agent_twice_is_ok() {
    let dna_hash = fixt!(DnaHash);
    let agent = fixt!(AgentPubKey).to_k2_agent();
    let db = test_conductor_db().to_db();
    let blocks = HolochainBlocks::new(dna_hash.clone(), db.clone());

    // First block.
    block_agent(&db, &dna_hash, &agent).await;

    // Second block should not error.
    block_agent(&db, &dna_hash, &agent).await;

    // Still blocked.
    assert!(blocks
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());
}

// Same conductor DB, but two different DNA hashes.
#[tokio::test(flavor = "multi_thread")]
async fn block_is_scoped_per_dna() {
    let agent = fixt!(AgentPubKey).to_k2_agent();
    let db = test_conductor_db().to_db();

    let dna_hash_1 = fixt!(DnaHash);
    let dna_hash_2 = fixt!(DnaHash);

    let blocks1 = HolochainBlocks::new(dna_hash_1.clone(), db.clone());
    let blocks2 = HolochainBlocks::new(dna_hash_2.clone(), db.clone());

    // Initially not blocked in either DNA.
    assert!(!blocks1
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());
    assert!(!blocks2
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());

    // Block in DNA1 only.
    block_agent(&db, &dna_hash_1, &agent).await;

    // Blocked in DNA1.
    assert!(blocks1
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());

    // Still not blocked in DNA2.
    assert!(!blocks2
        .is_blocked(BlockTarget::Agent(agent.clone()))
        .await
        .unwrap());

    // All-blocked checks respect DNA scoping too.
    assert!(blocks1
        .are_all_blocked(vec![BlockTarget::Agent(agent.clone())])
        .await
        .unwrap());
    assert!(!blocks2
        .are_all_blocked(vec![BlockTarget::Agent(agent.clone())])
        .await
        .unwrap());
}

async fn block_agent(db: &DbWrite<DbKindConductor>, dna_hash: &DnaHash, agent_id: &AgentId) {
    let cell_id = CellId::new(dna_hash.clone(), AgentPubKey::from_k2_agent(agent_id));
    let block_data = Block::new(
        holochain_types::prelude::BlockTarget::Cell(cell_id, CellBlockReason::App(vec![])),
        InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::MAX).unwrap(),
    );
    block(&db, block_data).await.unwrap();
}
