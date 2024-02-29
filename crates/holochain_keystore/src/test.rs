use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_lair_in_proc_start_time() {
    let passphrase = sodoken::BufRead::from(&b"passphrase"[..]);

    // -- the Interactive one -- //

    let tmp1 = tempdir::TempDir::new("test_lair_in_proc_start_time").unwrap();
    let mut path1 = tmp1.path().to_owned();
    path1.push("lair-config.yaml");

    println!("tmp1: {path1:?}");

    let start1 = std::time::Instant::now();

    let _client1 = crate::lair_keystore::spawn_lair_keystore_in_proc(
        &path1,
        passphrase.clone(),
        PwHashLimits::Interactive,
    )
    .await
    .unwrap();

    let interactive_time = start1.elapsed().as_secs_f64();

    println!("Interactive created in {} s", interactive_time);

    // -- the Minimum one -- //

    let tmp2 = tempdir::TempDir::new("test_lair_in_proc_start_time").unwrap();
    let mut path2 = tmp2.path().to_owned();
    path2.push("lair-config.yaml");

    println!("tmp2: {path2:?}");

    let start2 = std::time::Instant::now();

    let _client2 = crate::lair_keystore::spawn_lair_keystore_in_proc(
        &path2,
        passphrase,
        PwHashLimits::Minimum,
    )
    .await
    .unwrap();

    let minimum_time = start2.elapsed().as_secs_f64();

    println!("Minimum created in {} s", minimum_time);

    // -- assert it makes a difference -- //

    assert!(interactive_time > minimum_time);
}
