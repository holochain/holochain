#![cfg(feature = "wasmer_wamr")]

use crate::core::ribosome::error::RibosomeResult;
use holochain_wasmer_host::module::InstanceWithStore;
use holochain_zome_types::prelude::WasmZome;
use std::sync::Arc;
use tracing::warn;
use wasmer::Module;

// Metering is not supported in wasmer_wamr feature. This is a no-op.
pub fn reset_metering_points(_instance_with_store: Arc<InstanceWithStore>) {}

// Metering is not supported in wasmer_wamr feature. This is a no-op.
pub fn get_used_metering_points(_instance_with_store: Arc<InstanceWithStore>) -> u64 {
    0
}

// Use of precompiled and serialized modules is not supported in wasmer_wamr feature.
// If a preserialized_path is specified for the zome, it is ignored.
pub fn get_prebuilt_module(wasm_zome: &WasmZome) -> RibosomeResult<Option<Arc<Module>>> {
    if wasm_zome.preserialized_path.is_some() {
        warn!("A precompiled wasm path was specified but the feature flag 'wasmer_sys' must be enabled to support use of precompiled wasm modules. Ignoring the precompiled path.");
    }

    Ok(None)
}
