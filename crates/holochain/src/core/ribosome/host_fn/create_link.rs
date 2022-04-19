use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn create_link<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateLinkInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let CreateLinkInput {
                base_address,
                target_address,
                type_location,
                tag,
                chain_top_ordering,
            } = input;

            let zome = ribosome
                .dna_def()
                .get_integrity_zome(&type_location.zome)
                .map_err(|zome_error| WasmError::Host(zome_error.to_string()))?;

            // extract the zome position
            let zome_id = ribosome
                .zome_name_to_id(zome.zome_name())
                .expect("Failed to get ID for current zome");

            // Construct the link add
            let header_builder = builder::CreateLink::new(
                base_address,
                target_address,
                zome_id,
                type_location.link,
                tag,
            );

            let header_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the header into the source chain
                let header_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put(None, header_builder, None, chain_top_ordering)
                    .await?;
                Ok::<HeaderHash, RibosomeError>(header_hash)
            }))
            .map_err(|join_error| WasmError::Host(join_error.to_string()))?
            .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?;

            // return the hash of the committed link
            // note that validation is handled by the workflow
            // if the validation fails this commit will be rolled back by virtue of the DB transaction
            // being atomic
            Ok(header_hash)
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create_link".into(),
            )
            .to_string(),
        )),
    }
}

// we rely on the tests for get_links and get_link_details
