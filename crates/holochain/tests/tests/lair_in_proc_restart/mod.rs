use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain::sweettest::SweetRendezvous;
use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_keystore::MetaLairClient;
use holochain_wasm_test_utils::TestWasm;
use lair_keystore_api::dependencies::*;
use lair_keystore_api::in_proc_keystore::*;
use lair_keystore_api::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Written for https://github.com/holochain/lair/issues/120 to verify
/// that InProcKeystore still works after conductor restart
#[tokio::test(flavor = "multi_thread")]
async fn lair_in_proc_sql_pool_factory_restart() {
    // working temp dir
    let tmp = tempfile::tempdir().unwrap();

    // set up new lair keystore config
    let passphrase = Arc::new(Mutex::new(sodoken::LockedArray::from(
        b"passphrase".to_vec(),
    )));
    let config = Arc::new(
        hc_seed_bundle::PwHashLimits::Minimum
            .with_exec(|| LairServerConfigInner::new(tmp.path(), passphrase.clone()))
            .await
            .unwrap(),
    );

    let store_factory = lair_keystore::create_sql_pool_factory(
        tmp.path().join("store_file"),
        &config.database_salt,
    );

    let keystore = InProcKeystore::new(config, store_factory, passphrase)
        .await
        .unwrap();

    // print keystore config
    let keystore_config = keystore.get_config();
    println!("\n## keystore config ##\n{}", keystore_config);

    // set up conductor config to use the started keystore
    let conductor_config = ConductorConfig {
        keystore: KeystoreConfig::LairServerInProc {
            lair_root: Some(PathBuf::from(tmp.path()).into()),
        },
        ..Default::default()
    };

    let lair_client = keystore.new_client().await.unwrap();

    let meta_lair_client = MetaLairClient::from_client(lair_client).await.unwrap();

    let mut conductor = SweetConductor::create_with_defaults(
        conductor_config,
        Some(meta_lair_client),
        None::<Arc<dyn SweetRendezvous>>,
    )
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();

    let cell = app.cells().first().unwrap().clone();

    let _: String = conductor.call(&cell.zome("foo"), "foo", ()).await;

    conductor.shutdown().await;
    conductor.startup().await;

    // Test that zome calls still work after a restart
    let _: String = conductor.call(&cell.zome("foo"), "foo", ()).await;
}
