#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_demo");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_demo");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_demo";

use hdk::prelude::*;
super::wasm_common!();

/// hc_demo_cli integrity wasm bytes
pub const INTEGRITY_WASM: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/integrity/wasm32-unknown-unknown/release/hc_demo_cli.wasm"
));

/// hc_demo_cli coordinator wasm bytes
pub const COORDINATOR_WASM: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/coordinator/wasm32-unknown-unknown/release/hc_demo_cli.wasm"
));

use std::sync::Arc;
use holochain_types::prelude::*;

fn init_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// Execute the demo
pub async fn run_demo() {
    init_tracing();

    struct PubRendezvous;

    impl holochain::sweettest::SweetRendezvous for PubRendezvous {
        fn bootstrap_addr(&self) -> &str {
            "https://bootstrap.holo.host"
        }

        fn sig_addr(&self) -> &str {
            "wss://holotest.net"
        }
    }

    let rendezvous: holochain::sweettest::DynSweetRendezvous = Arc::new(PubRendezvous);

    let config = holochain::sweettest::SweetConductorConfig::standard();

    let keystore = holochain_keystore::spawn_mem_keystore().await.unwrap();

    let mut conductor = holochain::sweettest::SweetConductor::from_config_rendezvous_keystore(
        config, rendezvous, keystore,
    )
    .await;

    let i_wasm = DnaWasmHashed::from_content(DnaWasm {
        code: Arc::new(INTEGRITY_WASM.to_vec().into_boxed_slice()),
    }).await;
    println!("{i_wasm:#?}");
    let i_zome = IntegrityZomeDef::from(ZomeDef::Wasm(WasmZome::new(i_wasm.hash.clone())));

    let c_wasm = DnaWasmHashed::from_content(DnaWasm {
        code: Arc::new(COORDINATOR_WASM.to_vec().into_boxed_slice()),
    }).await;
    println!("{c_wasm:#?}");
    let c_zome = CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome::new(c_wasm.hash.clone())));

    let dna_def = DnaDefBuilder::default()
        .name("hc_demo_cli".to_string())
        .modifiers(
            DnaModifiersBuilder::default()
                .network_seed("".to_string())
                .origin_time(Timestamp::HOLOCHAIN_EPOCH)
                .build()
                .unwrap()
        )
        .integrity_zomes(vec![("integrity".into(), i_zome)])
        .coordinator_zomes(vec![("coordinator".into(), c_zome)])
        .build()
        .unwrap();
    println!("{dna_def:#?}");

    let dna_file = DnaFile::new(
        dna_def,
        vec![i_wasm.into_content(), c_wasm.into_content()],
    ).await;
    println!("{dna_file:#?}");

    let dna_with_role = holochain::sweettest::DnaWithRole::from((
        "hc_demo_cli".into(),
        dna_file,
    ));

    let app = conductor.setup_app(
        "hc_demo_cli",
        vec![&dna_with_role],
    ).await.unwrap();

    let cell = app.cells().get(0).unwrap().clone();
    println!("{:#?}", cell);

    let i_zome = cell.zome("integrity");
    println!("{i_zome:?}");
    let c_zome = cell.zome("coordinator");
    println!("{c_zome:?}");

    let handle = conductor.sweet_handle();
    let res: Record = handle.call(&c_zome, "create_file", File {
        desc: "yo".to_string(),
        data: UnsafeBytes::from(vec![0_u8, 1, 2, 3]).try_into().unwrap(),
    }).await;
    println!("{res:?}");

    conductor.shutdown().await;
}
