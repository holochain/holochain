use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::*;

use holochain_types::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn close_chain(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CloseChainInput,
) -> Result<ActionHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            // Construct the close chain action
            let action_builder =
                builder::CloseChain::new(input.new_dna_hash);

            let action_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the action into the source chain
                let action_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put_weightless(action_builder, None, ChainTopOrdering::Strict)
                    .await?;
                Ok::<ActionHash, RibosomeError>(action_hash)
            }))
                .map_err(|join_error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(join_error.to_string())).into()
                })?
                .map_err(|ribosome_error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(ribosome_error.to_string())).into()
                })?;

            // Return the hash of the chain close
            Ok(action_hash)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "close_chain".into()
            )
            .to_string()
        ))
            .into()),
    }
}
