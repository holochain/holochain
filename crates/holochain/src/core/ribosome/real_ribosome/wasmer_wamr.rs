use crate::core::ribosome::error::RibosomeResult;
use holochain_wasmer_host::module::InstanceWithStore;
use std::path::PathBuf;
use std::sync::Arc;
use wasmer::Module;

pub fn preserialized_module(_path: &PathBuf) -> RibosomeResult<Arc<Module>> {
    unimplemented!("The feature flag 'wasmer_sys' must be enabled to support compiling wasm");
}

// Metering is not supported in wasmer_wamr feature. This is a no-op.
pub fn reset_metering_points(_instance_with_store: Arc<InstanceWithStore>) {}

// Metering is not support in wasmer_wamr feature. This is a no-op.
pub fn get_used_metering_points(_instance_with_store: Arc<InstanceWithStore>) -> u64 {
    0
}
