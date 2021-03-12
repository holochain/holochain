use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_authored;
use crate::core::workflow::CallZomeWorkspace;
use holochain_wasmer_host::prelude::WasmError;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn create_link<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateLinkInput,
) -> Result<HeaderHash, WasmError> {
    let CreateLinkInput {
        base_address,
        target_address,
        tag,
    } = input;

    // extract the zome position
    let zome_id = ribosome
        .zome_to_id(&call_context.zome)
        .expect("Failed to get ID for current zome");

    // Construct the link add
    let header_builder = builder::CreateLink::new(base_address, target_address, zome_id, tag);

    let header_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
        let mut guard = call_context.host_access.workspace().write().await;
        let workspace: &mut CallZomeWorkspace = &mut guard;
        // push the header into the source chain
        let header_hash = workspace.source_chain.put(header_builder, None, None).await?;
        let element = workspace
            .source_chain
            .get_element(&header_hash)?
            .expect("Element we just put in SourceChain must be gettable");
        integrate_to_authored(
            &element,
            workspace.source_chain.elements(),
            &mut workspace.meta_authored,
        )
        .map_err(Box::new)?;
        Ok::<HeaderHash, RibosomeError>(header_hash)
    }))
    .map_err(|join_error| WasmError::Host(join_error.to_string()))?
    .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?;

    // return the hash of the committed link
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(header_hash)
}

// we rely on the tests for get_links and get_link_details
