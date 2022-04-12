use arbitrary::*;
use holo_hash::HasHash;
use holochain_cascade::test_utils::fill_db;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_p2p::dht_arc::DhtArcSet;
use holochain_types::dht_op::{DhtOp, DhtOpHashed};
use holochain_zome_types::{DnaDef, DnaDefHashed, NOISE};

use super::Spaces;

#[tokio::test(flavor = "multi_thread")]
async fn test_region_queries() {
    let mut u = Unstructured::new(&NOISE);
    let temp_dir = tempfile::TempDir::new().unwrap();
    let spaces = Spaces::new(&ConductorConfig {
        environment_path: temp_dir.path().to_path_buf().into(),
        ..Default::default()
    })
    .unwrap();
    let dna_def = DnaDefHashed::from_content_sync(DnaDef::arbitrary(&mut u).unwrap());
    let db = spaces.dht_db(dna_def.as_hash()).unwrap();
    for i in 0..100 {
        let op = DhtOpHashed::from_content_sync(DhtOp::arbitrary(&mut u).unwrap());
        fill_db(&db, op);
    }
    let arcset = DhtArcSet::Full;
    let regions = spaces
        .handle_fetch_op_regions(&dna_def, arcset)
        .await
        .unwrap();
    dbg!(&regions);
}
