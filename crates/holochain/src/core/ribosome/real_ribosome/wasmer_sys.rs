#![cfg(feature = "wasmer_sys")]

use crate::holochain_wasmer_host::module::WASM_METERING_LIMIT;
use holochain_wasmer_host::module::InstanceWithStore;
use holochain_zome_types::prelude::WasmZome;
use std::path::PathBuf;
use std::sync::Arc;
use wasmer::AsStoreMut;
use wasmer_middlewares::metering::{get_remaining_points, set_remaining_points, MeteringPoints};

pub fn reset_metering_points(instance_with_store: Arc<InstanceWithStore>) {
    let mut store_lock = instance_with_store.store.lock();
    let mut store_mut = store_lock.as_store_mut();
    set_remaining_points(
        &mut store_mut,
        instance_with_store.instance.as_ref(),
        WASM_METERING_LIMIT,
    );
}

pub fn get_used_metering_points(instance_with_store: Arc<InstanceWithStore>) -> u64 {
    let mut store_lock = instance_with_store.store.lock();
    let mut store_mut = store_lock.as_store_mut();

    match get_remaining_points(&mut store_mut, instance_with_store.instance.as_ref()) {
        MeteringPoints::Remaining(points) => WASM_METERING_LIMIT - points,
        MeteringPoints::Exhausted => WASM_METERING_LIMIT,
    }
}

pub fn get_preserialized_path(wasm_zome: &WasmZome) -> Option<&PathBuf> {
    wasm_zome.preserialized_path.as_ref()
}
