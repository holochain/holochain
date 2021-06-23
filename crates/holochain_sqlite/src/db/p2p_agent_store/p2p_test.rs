use crate::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::{KitsuneAgent, KitsuneSignature, KitsuneSpace};
use rand::Rng;
use std::sync::Arc;

fn rand_space() -> Arc<KitsuneSpace> {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    Arc::new(KitsuneSpace(data))
}

fn rand_agent() -> Arc<KitsuneAgent> {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    Arc::new(KitsuneAgent(data))
}

fn rand_signed_at_ms() -> u64 {
    let mut rng = rand::thread_rng();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    now - rng.gen_range(1000, 2000)
}

async fn rand_insert(db: &DbWrite, space: &Arc<KitsuneSpace>, agent: &Arc<KitsuneAgent>) {
    let mut rng = rand::thread_rng();

    let signed_at_ms = rand_signed_at_ms();
    let expires_at_ms = signed_at_ms + rng.gen_range(100, 200);

    let half_len = match rng.gen_range(0_u8, 5_u8) {
        0 => 0,
        1 => u32::MAX,
        _ => rng.gen_range(0, u32::MAX / 2),
    };

    let signed = AgentInfoSigned::sign(
        space.clone(),
        agent.clone(),
        half_len,
        vec![],
        signed_at_ms,
        expires_at_ms,
        |_| async { Ok(Arc::new(KitsuneSignature(vec![0; 64]))) },
    )
    .await
    .unwrap();

    p2p_put(db, &signed).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_agent_store_sanity() {
    let tmp_dir = tempdir::TempDir::new("p2p_agent_store_sanity").unwrap();

    let space = rand_space();

    let db = DbWrite::test(&tmp_dir, DbKind::P2pAgentStore(space.clone())).unwrap();

    let mut example_agent = rand_agent();

    for _ in 0..20 {
        example_agent = rand_agent();

        for _ in 0..3 {
            rand_insert(&db, &space, &example_agent).await;
        }
    }

    let mut con = db.connection_pooled().unwrap();

    // check that we only get 20 results
    let all = con.p2p_list().unwrap();
    assert_eq!(20, all.len());

    // make sure we can get our example result
    println!("after insert select all count: {}", all.len());
    let signed = con.p2p_get(&example_agent).unwrap();
    assert!(signed.is_some());

    // check that gossip query over full range returns 20 results
    let all = con
        .p2p_gossip_query(u64::MIN, u64::MAX, DhtArc::new(0, u32::MAX))
        .unwrap();
    assert_eq!(20, all.len());

    // check that gossip query over zero time returns zero results
    let all = con
        .p2p_gossip_query(u64::MIN, u64::MIN, DhtArc::new(0, u32::MAX))
        .unwrap();
    assert_eq!(0, all.len());

    // check that gossip query over zero arc returns zero results
    let all = con
        .p2p_gossip_query(u64::MIN, u64::MAX, DhtArc::new(0, 0))
        .unwrap();
    assert_eq!(0, all.len());

    // check that gossip query over half arc returns some but not all results
    let all = con
        .p2p_gossip_query(u64::MIN, u64::MAX, DhtArc::new(0, u32::MAX / 4))
        .unwrap();
    assert!(all.len() > 0 && all.len() < 20);

    // prune everything by expires time
    p2p_prune(&db).await.unwrap();

    // after prune, make sure all are pruned
    let all = con.p2p_list().unwrap();
    assert_eq!(0, all.len());

    // make sure our specific get also returns None
    println!("after prune_all select all count: {}", all.len());
    let signed = con.p2p_get(&example_agent).unwrap();
    assert!(signed.is_none());

    // clean up temp dir
    tmp_dir.close().unwrap();
}
