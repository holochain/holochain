use assert_cmd::prelude::*;
use holochain::sweettest::WsPollRecv;
use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_conductor_api::{AdminInterfaceConfig, AdminResponse};
use holochain_types::websocket::AllowedOrigins;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::dependencies::*;
use lair_keystore_api::ipc_keystore::*;
use lair_keystore_api::mem_store::*;
use lair_keystore_api::prelude::*;
use std::sync::Arc;

use super::test_utils::*;

const ADMIN_PORT: u16 = 12909;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_os = "macos",
    ignore = "processes fail to launch on macos, broken!"
)]
async fn test_new_lair_conductor_integration() {
    // working temp dir
    let tmp = tempfile::tempdir().unwrap();

    // set up new lair keystore config
    let passphrase = sodoken::BufRead::from(&b"passphrase"[..]);
    let config = Arc::new(
        hc_seed_bundle::PwHashLimits::Minimum
            .with_exec(|| LairServerConfigInner::new(tmp.path(), passphrase.clone()))
            .await
            .unwrap(),
    );

    // start the ipc keystore
    let keystore = IpcKeystoreServer::new(config, create_mem_store_factory(), passphrase)
        .await
        .unwrap();

    // print keystore config
    let keystore_config = keystore.get_config();
    println!("\n## keystore config ##\n{}", keystore_config);

    // set up conductor config to use the started keystore
    let conductor_config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port: ADMIN_PORT,
                allowed_origins: AllowedOrigins::Any,
            },
        }]),
        data_root_path: Some(tmp.path().to_owned().into()),
        keystore: KeystoreConfig::LairServer {
            connection_url: keystore_config.connection_url.clone().into(),
        },
        ..ConductorConfig::default()
    };

    // write the conductor config
    let conductor_config = serde_yaml::to_string(&conductor_config).unwrap();
    let mut cc_path = tmp.path().to_owned();
    cc_path.push("conductor_config.yml");
    tokio::fs::write(&cc_path, &conductor_config).await.unwrap();
    println!("\n## conductor config ##\n{}", conductor_config);

    // start a conductor using the new config
    let cmd = std::process::Command::cargo_bin("holochain").unwrap();
    let mut cmd = tokio::process::Command::from(cmd);
    cmd.arg("--structured")
        .arg("--config-path")
        .arg(cc_path)
        .arg("--piped")
        .env("RUST_LOG", "trace")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().unwrap();

    // we asked for the passphrase to be read from stdin pipe
    // provide it now
    let mut stdin = child.stdin.take().unwrap();
    use tokio::io::AsyncWriteExt;
    stdin.write_all(b"passphrase").await.unwrap();
    drop(stdin);

    // cribbed this from test_utils... probably something better would be better
    if let Ok(status) = tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await
    {
        panic!("failed to start holochain: {:?}", status);
    }

    let (mut client, rx) = websocket_client_by_port(ADMIN_PORT).await.unwrap();
    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    let agent_key = generate_agent_pub_key(&mut client, 15_000).await.unwrap();
    println!("GENERATED AGENT KEY: {}", agent_key);
    let mut agent_key_bytes = [0; 32];
    agent_key_bytes.copy_from_slice(agent_key.get_raw_32());
    println!("AGENT ED25519 PUBKEY: {:?}", agent_key_bytes);

    let store = keystore.store().await.unwrap();
    let entry = store
        .get_entry_by_ed25519_pub_key(agent_key_bytes.into())
        .await
        .unwrap();
    println!("AGENT_STORE_ENTRY: {:?}", entry);

    match &*entry {
        LairEntryInner::Seed { seed_info, .. } => {
            assert_eq!(*seed_info.ed25519_pub_key, agent_key_bytes);
        }
        oth => panic!("invalid entry type: {:?}", oth),
    }

    child.kill().await.unwrap();
}
