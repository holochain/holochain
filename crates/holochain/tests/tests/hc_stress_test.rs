use holochain::sweettest::*;
use holochain::test_utils::hc_stress_test::*;
use holochain_types::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn hc_stress_test_check_zome_functions() {
    // this is a sanity check to make sure the zome functions work
    // so that we can make more complex behavioral tests

    let conductor = SweetConductor::local_rendezvous().await;
    let dna = HcStressTest::test_dna(random_network_seed()).await;
    let mut test = HcStressTest::new(conductor, &[dna]).await;

    let rec = test.create_file(0, "hello world").await;
    println!("create: {:?}", rec);
    assert_eq!("hello world", HcStressTest::record_to_file_data(&rec),);

    let all = test.get_all_images(0).await;
    println!("all: {:?}", all);
    assert_eq!(1, all.len());

    for hash in all {
        let rec = test.get_file(0, hash.clone()).await.unwrap();
        println!("get: {hash:?}: {:?}", rec);
        assert_eq!("hello world", HcStressTest::record_to_file_data(&rec),);
    }
}

#[cfg(feature = "glacial_tests")]
#[tokio::test(flavor = "multi_thread")]
// NOTE: this test doesn't run correctly on one particular mac CI runner
#[cfg(not(target_os = "macos"))]
async fn hc_stress_test_3_min_behavior_1() {
    let test = LocalBehavior1::new();

    for _ in 0..6 {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        println!("{:#?}", &*test.lock().unwrap());
    }
}

#[cfg(feature = "glacial_tests")]
#[tokio::test(flavor = "multi_thread")]
// NOTE: this test doesn't run correctly on one particular mac CI runner
#[cfg(not(target_os = "macos"))]
async fn hc_stress_test_3_min_behavior_2() {
    let test = LocalBehavior2::new(4, 4);

    for _ in 0..6 {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        println!("{:#?}", &*test.lock().unwrap());
    }
}
