use holochain::sweettest::*;
use holochain::test_utils::hc_stress_test::*;
use holochain_types::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn check_zome_functions() {
    // this is a sanity check to make sure the zome functions work
    // so that we can make more complex behavioral tests

    let conductor = SweetConductor::from_standard_config().await;
    let dna = HcStressTest::test_dna(random_network_seed()).await;
    let mut test = HcStressTest::new(conductor, dna).await;

    let rec = test.create_file("hello world").await;
    println!("create: {:?}", rec);
    assert_eq!("hello world", HcStressTest::record_to_file_data(&rec),);

    let all = test.get_all_images().await;
    println!("all: {:?}", all);
    assert_eq!(1, all.len());

    for hash in all {
        let rec = test.get_file(hash.clone()).await;
        println!("get: {hash:?}: {:?}", rec);
        assert_eq!("hello world", HcStressTest::record_to_file_data(&rec),);
    }
}
