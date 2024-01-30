use std::sync::Arc;

use holochain_types::{access::{HostFnAccess, Permission}, app::InstalledApp};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::wasm_error;
use holochain_zome_types::clone::{ClonedCell, CreateCloneCellInput};
use wasmer::RuntimeError;
use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::{error::RibosomeError, CallContext, RibosomeT};

#[tracing::instrument(skip(_ribosome, call_context), fields(? call_context.zome, function = ? call_context.function_name))]
pub fn create_clone_cell<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateCloneCellInput,
) -> Result<ClonedCell, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let host_context = call_context.host_context();
            let conductor_handle = host_context.call_zome_handle();
            let app_id = input.app_id.clone();
            let installed_app: InstalledApp =
                tokio_helper::block_forever_on(async move {
                    conductor_handle.get_app(&app_id).await
                })
                .map_err(|conductor_error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
                })?;

            let current_cell_id = conductor_handle.cell_id();
            let target_cell_id = installed_app.role(&input.role_name).map_err(|conductor_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
            })?.cell_id();

            if current_cell_id != target_cell_id {
                // Mismatch between current zome being called and the target of the clone
                return Err(wasm_error!(WasmErrorInner::Host(
                    RibosomeError::CrossCellConductorCall(
                        target_cell_id.clone(),
                        current_cell_id.clone(),
                    )
                    .to_string(),
                )).into());
            }

            tokio_helper::block_forever_on(async move {
                conductor_handle.create_clone_cell(input)
                    .await
            }).map_err(|conductor_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create_clone_cell".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}
