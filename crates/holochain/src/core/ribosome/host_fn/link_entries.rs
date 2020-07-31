use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_cache;
use crate::core::{
    ribosome::{CallContext, RibosomeT},
    workflow::CallZomeWorkspace,
    SourceChainResult,
};
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::HeaderHash;
use holochain_zome_types::header::builder;
use holochain_zome_types::LinkEntriesInput;
use holochain_zome_types::LinkEntriesOutput;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn link_entries<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: LinkEntriesInput,
) -> RibosomeResult<LinkEntriesOutput> {
    let (base_address, target_address, tag) = input.into_inner();

    // extract the zome position
    let zome_id: holochain_zome_types::header::ZomeId = match ribosome
        .dna_file()
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &call_context.zome_name)
    {
        Some(index) => holochain_zome_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(call_context.zome_name.clone()))?,
    };

    // Construct the link add
    let header_builder = builder::LinkAdd::new(base_address, target_address, zome_id, tag);

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
            async move {
                let source_chain = &mut workspace.source_chain;
                // push the header into the source chain
                let header_hash = source_chain.put(header_builder, None).await?;
                let element = source_chain
                    .get_element(&header_hash)
                    .await?
                    .expect("Element we just put in SourceChain must be gettable");
                integrate_to_cache(
                    &element,
                    workspace.source_chain.elements(),
                    &mut workspace.cache_meta,
                )
                .await
                .map_err(Box::new)?;
                Ok(header_hash)
            }
            .boxed()
        };
    let header_hash =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        }))???;

    // return the hash of the committed link
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(LinkEntriesOutput::new(header_hash))
}
