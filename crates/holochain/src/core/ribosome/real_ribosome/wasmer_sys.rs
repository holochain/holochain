#![cfg(feature = "wasmer_sys")]

use crate::{
    core::ribosome::error::RibosomeResult, holochain_wasmer_host::module::WASM_METERING_LIMIT,
};
use holochain_wasmer_host::module::InstanceWithStore;
use holochain_zome_types::prelude::WasmZome;
use std::sync::Arc;
use wasmer::{AsStoreMut, Module};
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

pub fn get_prebuilt_module(wasm_zome: &WasmZome) -> RibosomeResult<Option<Arc<Module>>> {
    match &wasm_zome.preserialized_path {
        Some(path) => {
            let module = holochain_wasmer_host::module::get_ios_module_from_file(path.as_path())?;
            Ok(Some(Arc::new(module)))
        }
        None => Ok(None),
    }
}
