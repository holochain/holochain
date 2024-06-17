use crate::conductor::api::CellConductorReadHandle;
use holochain_types::app::{InstalledApp, InstalledAppId};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner, WasmHostError};
use holochain_zome_types::{call::RoleName, cell::CellId};
use wasmer::RuntimeError;

/// Check whether the current cell belongs to the app we're trying to perform a clone operation on.
/// If so, return the app id and role name that match the target cell. Otherwise, return an error.
///
/// This function takes the target cell to be cloned as an argument, and fetches the current cell from the call context
/// so that the check cannot accidentally be called on the wrong cell, permitting access to the wrong app.
pub fn check_clone_access(
    target_cell_id: &CellId,
    conductor_handle: &CellConductorReadHandle,
) -> Result<(InstalledAppId, RoleName), RuntimeError> {
    let current_cell_id = conductor_handle.cell_id();

    let installed_app: InstalledApp = tokio_helper::block_forever_on(async move {
        conductor_handle
            .find_app_containing_cell(current_cell_id)
            .await
    })
    .map_err(|conductor_error| -> RuntimeError {
        wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
    })?
    .ok_or::<RuntimeError>(
        wasm_error!(WasmErrorInner::Host(
            "App not found for current cell".to_string(),
        ))
        .into(),
    )?;

    // Check whether the current cell belongs to the app we're trying to perform a clone operation on.
    let matched_app_role = installed_app
        .roles()
        .iter()
        .find(|(_, app)| app.cell_id() == target_cell_id)
        .map(|(role_name, _)| role_name);

    if let Some(role_name) = matched_app_role {
        Ok((installed_app.id().clone(), role_name.clone()))
    } else {
        Err(wasm_error!(WasmErrorInner::Host(
            "Invalid request to modify a cell which belongs to another app".to_string(),
        ))
        .into())
    }
}
