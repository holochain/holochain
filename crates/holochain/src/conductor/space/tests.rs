use std::time::Duration;

use arbitrary::*;
use contrafact::Fact;
use holo_hash::HasHash;
use holochain_cascade::test_utils::fill_db;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_p2p::dht::hash::RegionHash;
use holochain_p2p::dht::region::RegionData;
use holochain_p2p::dht_arc::DhtArcSet;
use holochain_types::dht_op::facts::valid_dht_op;
use holochain_types::dht_op::{DhtOp, DhtOpHashed};
use holochain_types::prelude::*;
use holochain_zome_types::{DnaDef, DnaDefHashed, NOISE};
use rand::Rng;

use super::Spaces;

/// Test that `fetch_op_regions` returns regions which correctly describe
/// the set of ops in the database, and that `fetch_ops_by_region` returns the
/// entire set of ops
#[tokio::test(flavor = "multi_thread")]
async fn test_region_queries() {
    const NUM_OPS: usize = 100;

    let mut u = Unstructured::new(&NOISE);
    let temp_dir = tempfile::TempDir::new().unwrap();
    let spaces = Spaces::new(&ConductorConfig {
        environment_path: temp_dir.path().to_path_buf().into(),
        ..Default::default()
    })
    .unwrap();
    let keystore = test_keystore();
    let agent = keystore.new_sign_keypair_random().await.unwrap();
    let mut dna_def = DnaDef::arbitrary(&mut u).unwrap();
    let two_hrs_ago = (Timestamp::now() - Duration::from_secs(60 * 60 * 2)).unwrap();
    dna_def.origin_time = two_hrs_ago.clone();
    let dna_def = DnaDefHashed::from_content_sync(dna_def);
    let db = spaces.dht_db(dna_def.as_hash()).unwrap();
    let mut ops = vec![];
    for _ in 0..NUM_OPS {
        let mut op = DhtOp::arbitrary(&mut u).unwrap();
        *op.author_mut() = agent.clone();
        let mut fact = valid_dht_op(keystore.clone(), agent.clone());
        fact.satisfy(&mut op, &mut u);
        // timestamp is between 1 and 2 hours ago, which is the historical
        // window
        *op.timestamp_mut() =
            (two_hrs_ago + Duration::from_secs(rand::thread_rng().gen_range(0, 60 * 60))).unwrap();
        let mut op2 = op.clone();
        let op = DhtOpHashed::from_content_sync(op);
        fill_db(&db, op.clone());
        ops.push(op.clone());

        // also construct an op which is ahead of the historical time window,
        // to ensure that these ops don't get returned in region queries
        *op2.timestamp_mut() = (two_hrs_ago
            + Duration::from_secs(rand::thread_rng().gen_range(60 * 60, 2 * 60 * 60)))
        .unwrap();
        let op2 = DhtOpHashed::from_content_sync(op2);
        fill_db(&db, op2);
    }
    let arcset = DhtArcSet::Full;
    let regions = spaces
        .handle_fetch_op_regions(&dna_def, arcset)
        .await
        .unwrap();
    let region_sum: RegionData = regions.regions().map(|r| r.data).sum();
    let hash_sum = ops
        .into_iter()
        .map(|op| RegionHash::from_vec(op.as_hash().get_raw_39().to_vec()).unwrap())
        .sum();
    assert_eq!(region_sum.count as usize, NUM_OPS);
    assert_eq!(region_sum.hash, hash_sum);

    dbg!(regions.nonzero_regions().collect::<Vec<_>>());

    todo!("now make sure we can fetch all ops by using these regions");
}
