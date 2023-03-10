#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_demo");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_demo");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_demo";

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

fn init_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            tracing_subscriber::filter::EnvFilter::from_default_env(),
        )
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
        config,
        rendezvous,
        keystore,
    ).await;

    conductor.shutdown().await;
}
