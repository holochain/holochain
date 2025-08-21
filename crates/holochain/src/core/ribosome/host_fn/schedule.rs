use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: String,
) -> Result<(), RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            call_context
                .host_context()
                .workspace_write()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if write_workspace access is given")
                .scratch()
                .apply(|scratch| {
                    scratch.add_scheduled_fn(ScheduledFn::new(
                        call_context.zome.zome_name().clone(),
                        input.into(),
                    ));
                })
                .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?;
            Ok(())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "schedule".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}
