use crate::prelude::*;
use holo_hash::{DnaHash, AgentPubKey};
use rusqlite::*;
use rand::Rng;

fn rand_space() -> DnaHash {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    DnaHash::from_raw_36(data)
}

fn rand_agent() -> AgentPubKey {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    AgentPubKey::from_raw_36(data)
}

fn rand_signed_at_ms() -> u64 {
    let mut rng = rand::thread_rng();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    now - rng.gen_range(100, 1000)
}

fn rand_insert(tx: &Transaction, space: &DnaHash, agent: &AgentPubKey) {
    let mut rng = rand::thread_rng();

    let signed_at_ms = rand_signed_at_ms();
    let expires_at_ms = signed_at_ms + rng.gen_range(100, 200);

    let mut s_1 = None;
    let mut e_1 = None;
    let mut s_2 = None;
    let mut e_2 = None;

    if rng.gen() {
        if rng.gen() {
            s_1 = Some(rng.gen_range(u32::MAX / 4, u32::MAX / 2));
            e_1 = Some(rng.gen_range(u32::MAX / 2 + 1, (u32::MAX / 4) * 3));
        } else {
            s_1 = Some(0);
            e_1 = Some(rng.gen_range(u32::MAX / 4, u32::MAX / 2));
            s_2 = Some(rng.gen_range(u32::MAX / 2 + 1, (u32::MAX / 4) * 3));
            e_2 = Some(u32::MAX);
        }
    }

    tx.p2p_insert(P2pRecordRef {
        space,
        agent,
        signed_at_ms,
        expires_at_ms,
        encoded: &[],
        storage_center_loc: 0,  // - these are not used yet
        storage_half_length: 0, // - these are not used yet
        storage_start_1: s_1,
        storage_end_1: e_1,
        storage_start_2: s_2,
        storage_end_2: e_2,
    }).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_store_sanity() {
    let tmp_dir = tempdir::TempDir::new("p2p_store_sanity").unwrap();
    let db = DbWrite::test(&tmp_dir, DbKind::P2p).unwrap();
    let mut con = db.connection_pooled().unwrap();

    let space = rand_space();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    con.with_commit(|writer| {
        for _ in 0..20 {
            let agent = rand_agent();

            for _ in 0..3 {
                rand_insert(writer, &space, &agent);
            }
        }

        let all = writer.p2p_select_all(&space).unwrap();
        assert_eq!(20, all.len());
        println!("after insert select all count: {}", all.len());
        println!("{:#?}", all.into_iter().map(|r| r.signed_at_ms).collect::<Vec<_>>());

        DatabaseResult::Ok(())
    }).unwrap();

    con.with_commit(|writer| {
        // prune duplicates, but not any expirations
        writer.prune(now - 2000).unwrap();

        let all = writer.p2p_select_all(&space).unwrap();
        assert_eq!(20, all.len());
        println!("after prune select all count: {}", all.len());
        println!("{:#?}", all.into_iter().map(|r| r.signed_at_ms).collect::<Vec<_>>());

        DatabaseResult::Ok(())
    }).unwrap();

    con.with_commit(|writer| {
        // prune everything by expires time
        writer.prune(now + 2000).unwrap();

        let all = writer.p2p_select_all(&space).unwrap();
        assert_eq!(0, all.len());
        println!("after prune_all select all count: {}", all.len());

        DatabaseResult::Ok(())
    }).unwrap();

    tmp_dir.close().unwrap();
}
