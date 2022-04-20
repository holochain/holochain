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
/// entire set of ops.
///
/// Constructs 100 ops in the historical time window, and 100 ops in the recent
/// time window, the latter of which will be ignored. Calculates the region set
/// for the full arc across all of history, and ensures that the regions
/// fully cover all 100 ops.
#[tokio::test(flavor = "multi_thread")]
async fn test_region_queries() {
    const NUM_OPS: usize = 100;

    let mut u = Unstructured::new(&NOISE);
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();

    let spaces = Spaces::new(&ConductorConfig {
        environment_path: path.into(),
        ..Default::default()
    })
    .unwrap();
    let keystore = test_keystore();
    let agent = keystore.new_sign_keypair_random().await.unwrap();
    let two_hrs_ago = (Timestamp::now() - Duration::from_secs(60 * 60 * 2)).unwrap();

    // - The origin time is two hours ago
    let mut dna_def = DnaDef::arbitrary(&mut u).unwrap();
    dna_def.origin_time = two_hrs_ago.clone();

    // Builds an arbitrary valid op at the given timestamp
    let mut arbitrary_valid_op = |timestamp: Timestamp| -> DhtOp {
        let mut op = DhtOp::arbitrary(&mut u).unwrap();
        *op.author_mut() = agent.clone();
        let mut fact = valid_dht_op(keystore.clone(), agent.clone());
        fact.satisfy(&mut op, &mut u);
        *op.timestamp_mut() = timestamp;
        op
    };

    let dna_def = DnaDefHashed::from_content_sync(dna_def);
    let topo = dna_def.topology();
    let db = spaces.dht_db(dna_def.as_hash()).unwrap();
    let mut ops = vec![];

    // - Check that we have no ops to begin with
    let region_set = spaces
        .handle_fetch_op_regions(&dna_def, DhtArcSet::Full)
        .await
        .unwrap();
    let region_sum: RegionData = region_set.regions().map(|r| r.data).sum();
    assert_eq!(region_sum.count as usize, 0);

    let min_ms = 1000 * 60;
    let hour_ms = min_ms * 60;

    for _ in 0..NUM_OPS {
        // timestamp is between 1 and 2 hours ago, which is the historical
        // window
        let op = arbitrary_valid_op(
            (two_hrs_ago + Duration::from_millis(rand::thread_rng().gen_range(0, hour_ms)))
                .unwrap(),
        );
        let op = DhtOpHashed::from_content_sync(op);
        fill_db(&db, op.clone());
        ops.push(op.clone());

        // also construct ops which are in the recent time window,
        // to test that these ops don't get returned in region queries.
        // There is a 5 min overlap between historical and recent time windows
        // (1 time quantum) so we make sure to account for that here also.
        let op2 = arbitrary_valid_op(
            (two_hrs_ago
                + Duration::from_millis(
                    hour_ms + rand::thread_rng().gen_range(5 * min_ms, hour_ms),
                ))
            .unwrap(),
        );
        let op2 = DhtOpHashed::from_content_sync(op2);
        fill_db(&db, op2);
    }
    let region_set = spaces
        .handle_fetch_op_regions(&dna_def, DhtArcSet::Full)
        .await
        .unwrap();

    // - Check that the aggregate of all region data matches expectations
    let region_sum: RegionData = region_set.regions().map(|r| r.data).sum();
    let hash_sum = ops
        .iter()
        .map(|op| RegionHash::from_vec(op.as_hash().get_raw_39().to_vec()).unwrap())
        .sum();
    assert_eq!(region_sum.count as usize, NUM_OPS);
    assert_eq!(region_sum.hash, hash_sum);

    let mut fetched_ops: Vec<_> = spaces
        .handle_fetch_op_data_by_regions(
            dna_def.as_hash(),
            region_set
                .regions()
                .map(|r| r.coords.to_bounds(&topo))
                .collect(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|(hash, _)| hash)
        .collect();

    let mut inserted_ops: Vec<_> = ops.into_iter().map(|op| op.into_hash()).collect();
    fetched_ops.sort();
    inserted_ops.sort();

    assert_eq!(fetched_ops.len(), NUM_OPS);
    assert_eq!(inserted_ops, fetched_ops);
}
