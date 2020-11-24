use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_authored;
use crate::core::workflow::CallZomeWorkspace;
use crate::core::SourceChainResult;
use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::header::builder;
use holochain_zome_types::CreateLinkInput;
use holochain_zome_types::CreateLinkOutput;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn create_link<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateLinkInput,
) -> RibosomeResult<CreateLinkOutput> {
    let (base_address, target_address, tag) = input.into_inner();

    // extract the zome position
    let zome_id = ribosome.zome_to_id(&call_context.zome)?;

    // Construct the link add
    let header_builder = builder::CreateLink::new(base_address, target_address, zome_id, tag);

    let header_hash =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            let mut guard = call_context.host_access.workspace().write().await;
            let workspace: &mut CallZomeWorkspace = &mut guard;
            // push the header into the source chain
            let header_hash = workspace.source_chain.put(header_builder, None).await?;
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
            SourceChainResult::Ok(header_hash)
        }))??;

    // return the hash of the committed link
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(CreateLinkOutput::new(header_hash))
}

// we rely on the tests for get_links and get_link_details
