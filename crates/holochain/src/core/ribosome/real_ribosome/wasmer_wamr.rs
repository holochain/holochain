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
