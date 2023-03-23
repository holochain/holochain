#[cfg(feature = "build_demo")]
compile_error!("feature build_demo is incompatible with build_integrity_wasm");

#[cfg(feature = "build_coordinator_wasm")]
compile_error!("feature build_coordinator_wasm is incompatible with build_integrity_wasm");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_integrity_wasm";

use hdi::prelude::*;

super::wasm_common!();
