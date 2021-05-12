use crate::db::PConn;
use crate::prelude::*;
use kitsune_p2p::agent_store::{AgentInfo, AgentInfoSigned, AgentMetaInfo};
use kitsune_p2p::{KitsuneAgent, KitsuneSignature, KitsuneSpace};
use rand::Rng;
use std::sync::Arc;

fn rand_space() -> KitsuneSpace {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    KitsuneSpace(data)
}

fn rand_agent() -> KitsuneAgent {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    KitsuneAgent(data)
}

fn rand_signed_at_ms() -> u64 {
    let mut rng = rand::thread_rng();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    now - rng.gen_range(1000, 2000)
}

fn rand_insert(con: &mut PConn, space: &KitsuneSpace, agent: &KitsuneAgent) {
    use std::convert::TryInto;

    let mut rng = rand::thread_rng();

    let signed_at_ms = rand_signed_at_ms();
    let expires_after_ms = rng.gen_range(100, 200);

    let info = AgentInfo::new(
        space.clone(),
        agent.clone(),
        vec![],
        signed_at_ms,
        expires_after_ms,
    );

    let half_len = match rng.gen_range(0_u8, 5_u8) {
        0 => 0,
        1 => u32::MAX,
        _ => rng.gen_range(0, u32::MAX / 2),
    };

    let info = info
        .with_meta_info(AgentMetaInfo {
            dht_storage_arc_half_length: half_len,
        })
        .unwrap();

    let signed = AgentInfoSigned::try_new(
        agent.clone(),
        KitsuneSignature(vec![0; 64]),
        (&info).try_into().unwrap(),
    )
    .unwrap();

    con.p2p_put(&signed).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_store_sanity() {
    let tmp_dir = tempdir::TempDir::new("p2p_store_sanity").unwrap();

    let space = rand_space();

    let db = DbWrite::test(&tmp_dir, DbKind::P2p(Arc::new(space.clone()))).unwrap();
    let mut con = db.connection_pooled().unwrap();

    let mut example_agent = rand_agent();

    for _ in 0..20 {
        example_agent = rand_agent();

        for _ in 0..3 {
            rand_insert(&mut con, &space, &example_agent);
        }
    }

    let all = con.p2p_list().unwrap();
    assert_eq!(20, all.len());
    println!("after insert select all count: {}", all.len());
    let signed = con.p2p_get(&example_agent).unwrap();
    assert!(signed.is_some());

    // prune everything by expires time
    con.p2p_prune().unwrap();

    let all = con.p2p_list().unwrap();
    assert_eq!(0, all.len());
    println!("after prune_all select all count: {}", all.len());
    let signed = con.p2p_get(&example_agent).unwrap();
    assert!(signed.is_none());

    tmp_dir.close().unwrap();
}
