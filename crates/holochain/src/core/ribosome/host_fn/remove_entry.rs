use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_cache;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::HeaderHash;
use holochain_zome_types::header::builder;
use holochain_zome_types::RemoveEntryInput;
use holochain_zome_types::RemoveEntryOutput;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn remove_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: RemoveEntryInput,
) -> RibosomeResult<RemoveEntryOutput> {
    let removes_address = input.into_inner();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
            async move {
                let source_chain = &mut workspace.source_chain;
                let header_builder = builder::ElementDelete { removes_address };
                let header_hash = source_chain.put(header_builder, None).await?;
                let element = source_chain
                    .get_element(&header_hash)
                    .await?
                    .expect("Element we just put in SourceChain must be gettable");
                integrate_to_cache(
                    &element,
                    &mut workspace.cache_cas,
                    &mut workspace.cache_meta,
                )
                .await
                .map_err(Box::new)?;
                Ok(header_hash)
            }
            .boxed()
        };

    // handle timeouts at the source chain layer
    let header_address =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        }))???;

    Ok(RemoveEntryOutput::new(header_address))
}
