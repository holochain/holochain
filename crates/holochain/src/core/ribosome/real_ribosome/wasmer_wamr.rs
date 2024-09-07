#![cfg(feature = "wasmer_wamr")]

use crate::core::ribosome::error::RibosomeResult;
use holochain_wasmer_host::module::InstanceWithStore;
use std::path::PathBuf;
use std::sync::Arc;
use wasmer::Module;

// Metering is not supported in wasmer_wamr feature. This is a no-op.
pub fn reset_metering_points(_instance_with_store: Arc<InstanceWithStore>) {}

// Metering is not support in wasmer_wamr feature. This is a no-op.
pub fn get_used_metering_points(_instance_with_store: Arc<InstanceWithStore>) -> u64 {
    0
}

// Use of precompiled and serialized modules is not support in wasmer_wamr feature.
pub fn get_preserialized_path(wasm_zome: &WasmZome) -> Option<&PathBuf> {
    None
}
