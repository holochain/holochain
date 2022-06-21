use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::*;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn create_link<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateLinkInput,
) -> Result<ActionHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let CreateLinkInput {
                base_address,
                target_address,
                zome_id,
                link_type,
                tag,
                chain_top_ordering,
            } = input;

            // Construct the link add
            let action_builder =
                builder::CreateLink::new(base_address, target_address, zome_id, link_type, tag);

            let action_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the action into the source chain
                let action_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put(action_builder, None, chain_top_ordering)
                    .await?;
                Ok::<ActionHash, RibosomeError>(action_hash)
            }))
            .map_err(|join_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(join_error.to_string())).into()
            })?
            .map_err(|ribosome_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(ribosome_error.to_string())).into()
            })?;

            // return the hash of the committed link
            // note that validation is handled by the workflow
            // if the validation fails this commit will be rolled back by virtue of the DB transaction
            // being atomic
            Ok(action_hash)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create_link".into()
            )
            .to_string()
        ))
        .into()),
    }
}

// we rely on the tests for get_links and get_link_details
