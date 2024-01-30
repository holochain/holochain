use std::sync::Arc;

use holochain_types::access::{HostFnAccess, Permission};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::wasm_error;
use holochain_zome_types::clone::{ClonedCell, EnableCloneCellInput};
use wasmer::RuntimeError;
use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::{check_clone_access::check_clone_access, error::RibosomeError, CallContext, RibosomeT};

#[tracing::instrument(skip(_ribosome, call_context), fields(? call_context.zome, function = ? call_context.function_name))]
pub fn enable_clone_cell<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: EnableCloneCellInput,
) -> Result<ClonedCell, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let host_context = call_context.host_context();

            let conductor_handle = host_context.call_zome_handle();
            check_clone_access(&input.app_id, conductor_handle)?;

            tokio_helper::block_forever_on(async move {
                conductor_handle.enable_clone_cell(input)
                    .await
            }).map_err(|conductor_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(conductor_error.to_string())).into()
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "enable_clone_cell".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}
