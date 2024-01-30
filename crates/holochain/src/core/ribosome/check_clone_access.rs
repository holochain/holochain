use holochain_types::app::{InstalledApp, InstalledAppId};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner};
use wasmer::RuntimeError;
use crate::conductor::api::CellConductorReadHandle;
use super::error::RibosomeError;

pub fn check_clone_access(target_app_id: &InstalledAppId, conductor_handle: &CellConductorReadHandle) -> Result<(), RuntimeError> {
    let app_id = target_app_id.clone();
    let installed_app: InstalledApp =
        tokio_helper::block_forever_on(async move { conductor_handle.get_app(&app_id).await })
            .map_err(|conductor_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
            })?;

    // Check whether the current cell belongs to the app we're trying to perform a clone operation on.
    let current_cell_id = conductor_handle.cell_id();
    let current_belongs_to_app = installed_app
        .roles()
        .values()
        .find(|r| r.cell_id() == current_cell_id)
        .is_some();

    if !current_belongs_to_app {
        return Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::InvalidCloneTarget(target_app_id.clone(), current_cell_id.clone(),)
                .to_string(),
        )).into());
    }

    Ok(())
}
