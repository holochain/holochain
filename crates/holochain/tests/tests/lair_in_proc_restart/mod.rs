use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_types::websocket::AllowedOrigins;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::dependencies::*;
use lair_keystore_api::in_proc_keystore::*;
use lair_keystore_api::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

const ADMIN_PORT: u16 = 12909;

/// Written for https://github.com/holochain/lair/issues/120 to verify
/// that InProcKeystore still works after conductor restart
#[tokio::test(flavor = "multi_thread")]
async fn lair_in_proc_sql_pool_factory_restart() {
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

    let store_factory = lair_keystore::create_sql_pool_factory(
        tmp.path().join("store_file"),
        &*&config.database_salt,
    );

    // start the ipc keystore
    let keystore = InProcKeystore::new(config, store_factory, passphrase)
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
        data_root_path: None,
        keystore: KeystoreConfig::LairServerInProc {
            lair_root: Some(PathBuf::from(tmp.path()).into()),
        },
        ..Default::default()
    };

    let mut conductor = SweetConductor::from_config(conductor_config).await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();

    let cell = app.cells().first().unwrap().clone();

    let _: String = conductor.call(&cell.zome("foo"), "foo", ()).await;

    conductor.shutdown().await;
    conductor.startup().await;

    // Test that zome calls still work after a restart
    let _: String = conductor.call(&cell.zome("foo"), "foo", ()).await;
}
