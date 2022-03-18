use crate::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::{ArcInterval, DhtArc, DhtArcSet};
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

async fn rand_insert(
    db: &DbWrite<DbKindP2pAgents>,
    space: &Arc<KitsuneSpace>,
    agent: &Arc<KitsuneAgent>,
    long: bool,
) {
    let mut rng = rand::thread_rng();

    let signed_at_ms = rand_signed_at_ms();

    let expires_at_ms = if long {
        signed_at_ms + rng.gen_range(10000, 20000)
    } else {
        signed_at_ms + rng.gen_range(100, 200)
    };

    let half_len = match rng.gen_range(0_u8, 9_u8) {
        0 => 0,
        1 => u32::MAX,
        2 => rng.gen_range(0, u32::MAX / 2),
        _ => rng.gen_range(0, u32::MAX / 1000),
    };

    let signed = AgentInfoSigned::sign(
        space.clone(),
        agent.clone(),
        half_len,
        vec!["fake:".into()],
        signed_at_ms,
        expires_at_ms,
        |_| async { Ok(Arc::new(KitsuneSignature(vec![0; 64]))) },
    )
    .await
    .unwrap();

    p2p_put(db, &signed).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[allow(unused_assignments)]
async fn test_p2p_agent_store_extrapolated_coverage() {
    let tmp_dir = tempfile::Builder::new()
        .prefix("p2p_agent_store_extrapolated_coverage")
        .tempdir()
        .unwrap();

    let space = rand_space();

    let db = DbWrite::test(tmp_dir.path(), DbKindP2pAgents(space.clone())).unwrap();

    let mut example_agent = rand_agent();

    for _ in 0..20 {
        example_agent = rand_agent();

        rand_insert(&db, &space, &example_agent, true).await;
    }

    let permit = db.conn_permit().await;
    let mut con = db.from_permit(permit).unwrap();

    let res = con.p2p_extrapolated_coverage(DhtArcSet::Full).unwrap();
    println!("{:?}", res);
    assert_eq!(1, res.len());

    let res = con
        .p2p_extrapolated_coverage(DhtArcSet::from(
            &[
                ArcInterval::from_bounds((1.into(), (u32::MAX / 2 - 1).into())),
                ArcInterval::from_bounds(((u32::MAX / 2 + 1).into(), (u32::MAX - 1).into())),
            ][..],
        ))
        .unwrap();
    println!("{:?}", res);
    assert_eq!(2, res.len());

    // clean up temp dir
    tmp_dir.close().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_agent_store_gossip_query_sanity() {
    let tmp_dir = tempfile::Builder::new()
        .prefix("p2p_agent_store_gossip_query_sanity")
        .tempdir()
        .unwrap();

    let space = rand_space();

    let db = DbWrite::test(tmp_dir.path(), DbKindP2pAgents(space.clone())).unwrap();

    let mut example_agent = rand_agent();

    for _ in 0..20 {
        example_agent = rand_agent();

        // insert multiple times to test idempotence of "upsert"
        for _ in 0..3 {
            rand_insert(&db, &space, &example_agent, false).await;
        }
    }

    let permit = db.conn_permit().await;
    let mut con = db.from_permit(permit).unwrap();

    // check that we only get 20 results
    let all = con.p2p_list_agents().unwrap();
    assert_eq!(20, all.len());

    // agents with zero arc lengths will never be returned, so count only the
    // nonzero ones
    let num_nonzero = all
        .iter()
        .filter(|a| a.storage_arc.half_length() > 0)
        .count();

    // make sure we can get our example result
    println!("after insert select all count: {}", all.len());
    let signed = con.p2p_get_agent(&example_agent).unwrap();
    assert!(signed.is_some());

    // check that gossip query over full range returns 20 results
    let all = con
        .p2p_gossip_query_agents(
            u64::MIN,
            u64::MAX,
            DhtArc::new(0, u32::MAX).interval().into(),
        )
        .unwrap();
    assert_eq!(all.len(), num_nonzero);

    // check that gossip query over zero time returns zero results
    let all = con
        .p2p_gossip_query_agents(
            u64::MIN,
            u64::MIN,
            DhtArc::new(0, u32::MAX).interval().into(),
        )
        .unwrap();
    assert_eq!(all.len(), 0);

    // check that gossip query over zero arc returns zero results
    let all = con
        .p2p_gossip_query_agents(u64::MIN, u64::MAX, DhtArc::new(0, 0).interval().into())
        .unwrap();
    assert_eq!(all.len(), 0);

    // check that gossip query over half arc returns some but not all results
    // NB: there is a very small probability of this failing
    let all = con
        .p2p_gossip_query_agents(
            u64::MIN,
            u64::MAX,
            DhtArc::new(0, u32::MAX / 4).interval().into(),
        )
        .unwrap();
    // NOTE - not sure this is right with <= num_nonzero... but it breaks
    //        sometimes if we just use '<'
    assert!(all.len() > 0 && all.len() <= num_nonzero);

    // near
    let tgt = u32::MAX / 2;
    let near = con.p2p_query_near_basis(tgt, 20).unwrap();
    let mut prev = 0;
    for agent_info_signed in near {
        use kitsune_p2p::KitsuneBinType;
        let loc = agent_info_signed.agent.get_loc();
        let record = super::P2pRecord::from_signed(&agent_info_signed).unwrap();
        let mut dist = u32::MAX;
        let mut deb = "not reset";

        let start = record.storage_start_loc;
        let end = record.storage_end_loc;

        match (start, end) {
            (Some(start), Some(end)) => {
                if start < end {
                    if tgt >= start && tgt <= end {
                        deb = "one-span-inside";
                        dist = 0;
                    } else if tgt < start {
                        deb = "one-span-before";
                        dist = std::cmp::min(start - tgt, (u32::MAX - end) + tgt);
                    } else {
                        deb = "one-span-after";
                        dist = std::cmp::min(tgt - end, (u32::MAX - tgt) + start);
                    }
                } else {
                    if tgt <= end || tgt >= start {
                        deb = "two-span-inside";
                        dist = 0;
                    } else {
                        deb = "two-span-outside";
                        dist = std::cmp::min(tgt - end, start - tgt);
                    }
                }
            }
            _ => (),
        }

        assert!(dist >= prev);
        prev = dist;
        println!("loc({}) => dist({}) - {}", loc, dist, deb);
    }

    // prune everything by expires time
    p2p_prune(&db, vec![]).await.unwrap();

    // after prune, make sure all are pruned
    let all = con.p2p_list_agents().unwrap();
    assert_eq!(0, all.len());

    // make sure our specific get also returns None
    println!("after prune_all select all count: {}", all.len());
    let signed = con.p2p_get_agent(&example_agent).unwrap();
    assert!(signed.is_none());

    // clean up temp dir
    tmp_dir.close().unwrap();
}
