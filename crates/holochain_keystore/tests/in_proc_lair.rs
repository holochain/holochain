#![cfg(unix)]

use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use holochain_keystore::AgentPubKeyExt;
use std::sync::{Arc, Mutex};

/// Lair root whose `<root>/socket` path is well past `sun_path`, like an iOS
/// app container.
fn long_lair_root(tmp: &std::path::Path) -> std::path::PathBuf {
    let mut root = tmp.to_path_buf();
    while root.join("socket").as_os_str().len() < 200 {
        root.push("0123456789abcdef0123456789abcdef");
    }
    root
}

// Regression test for <https://github.com/holochain/holochain/issues/5977>.
//
// `sockaddr_un.sun_path` is 104 bytes on macOS and 108 on Linux, while an iOS
// app container path is already longer than that before `socket` is appended.
// The in-proc keystore must therefore never bind a unix domain socket.
#[tokio::test(flavor = "multi_thread")]
async fn in_proc_keystore_boots_from_a_long_lair_root() {
    let tmp = tempfile::tempdir().unwrap();
    let lair_root = long_lair_root(tmp.path());
    tokio::fs::create_dir_all(&lair_root).await.unwrap();
    let config_path = lair_root.join("lair-keystore-config.yaml");

    // pre-fix this aborted binding `<lair_root>/socket` with
    // `Lair({"error":"InvalidInput","message":"path must be shorter than SUN_LEN"})`
    let keystore = spawn_lair_keystore_in_proc(&config_path, passphrase())
        .await
        .expect("in-proc lair must not require a unix domain socket");

    // config and sqlite store were created under the long path
    assert!(config_path.is_file());
    assert!(lair_root.join("store_file").is_file());

    // and the in-proc connection can actually sign
    let agent = keystore.new_sign_keypair_random().await.unwrap();
    let data: Arc<[u8]> = b"long path".to_vec().into();
    let sig = agent.sign_raw(&keystore, data.clone()).await.unwrap();
    assert!(agent.verify_signature_raw(&sig, data).await.unwrap());
}

/// Each conductor owns its own data root, so in-proc keystores on distinct
/// roots must be able to run side by side.
#[tokio::test(flavor = "multi_thread")]
async fn in_proc_keystores_run_in_parallel_on_distinct_roots() {
    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let config_a = tmp_a.path().join("lair-keystore-config.yaml");
    let config_b = tmp_b.path().join("lair-keystore-config.yaml");

    let (a, b) = tokio::join!(
        spawn_lair_keystore_in_proc(&config_a, passphrase()),
        spawn_lair_keystore_in_proc(&config_b, passphrase()),
    );
    let keystores = [
        a.expect("first in-proc keystore"),
        b.expect("second in-proc keystore"),
    ];

    for keystore in keystores {
        let agent = keystore.new_sign_keypair_random().await.unwrap();
        let data: Arc<[u8]> = b"parallel".to_vec().into();
        let sig = agent.sign_raw(&keystore, data.clone()).await.unwrap();
        assert!(agent.verify_signature_raw(&sig, data).await.unwrap());
    }
}

/// Each conductor owns its own data root, so in-proc keystores on distinct
/// roots must be able to run side by side.
#[tokio::test(flavor = "multi_thread")]
async fn in_proc_keystores_can_be_restarted() {
    lair_keystore::dependencies::tracing_subscriber::fmt::init();
    let tmp = tempfile::tempdir().unwrap();
    let lair_root = tmp.path().to_path_buf().join("socket");
    tokio::fs::create_dir_all(&lair_root).await.unwrap();
    let config_path = lair_root.join("lair-keystore-config.yaml");

    let keystore = spawn_lair_keystore_in_proc(&config_path, passphrase())
        .await
        .unwrap();

    let agent = keystore.new_sign_keypair_random().await.unwrap();
    keystore
        .sign(agent.clone(), b"some_data".to_vec().into())
        .await
        .unwrap();

    keystore.shutdown().await.unwrap();
    drop(keystore);

    let keystore = spawn_lair_keystore_in_proc(&config_path, passphrase())
        .await
        .unwrap();

    let agent = keystore.new_sign_keypair_random().await.unwrap();
    keystore
        .sign(agent.clone(), b"some_data".to_vec().into())
        .await
        .unwrap();
}

fn passphrase() -> Arc<Mutex<sodoken::LockedArray>> {
    Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"passphrase".to_vec(),
    )))
}
