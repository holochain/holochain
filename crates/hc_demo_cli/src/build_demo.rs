#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_demo");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_demo");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_demo";

/// hc_demo_cli integrity wasm bytes
pub const INTEGRITY_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/integrity/wasm32-unknown-unknown/release/hc_demo_cli.wasm"));

/// hc_demo_cli coordinator wasm bytes
pub const COORDINATOR_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coordinator/wasm32-unknown-unknown/release/hc_demo_cli.wasm"));
