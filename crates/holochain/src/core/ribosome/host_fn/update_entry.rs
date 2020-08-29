use super::{commit_entry::extract_entry_def, delete_entry::get_original_address};
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::{
    ribosome::RibosomeT,
    workflow::{integrate_dht_ops_workflow::integrate_to_cache, CallZomeWorkspace},
    SourceChainResult,
};
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_zome_types::UpdateEntryInput;
use holochain_zome_types::{
    header::{builder, AppEntryType, EntryType},
    UpdateEntryOutput,
};
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn update_entry<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: UpdateEntryInput,
) -> RibosomeResult<UpdateEntryOutput> {
    // destructure the args out into an app type def id and entry
    let (entry_def_id, entry, original_header_hash) = input.into_inner();

    // build the entry hash
    let async_entry = entry.clone();
    let entry_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        holochain_types::entry::EntryHashed::from_content_sync(async_entry)
    })
    .into_hash();

    // extract the zome position
    let header_zome_id = ribosome.zome_name_to_id(&call_context.zome_name)?;

    // extract the entry defs for a zome
    let (header_entry_def_id, entry_visibility) =
        extract_entry_def(ribosome, call_context.clone(), entry_def_id)?;

    let app_entry_type = AppEntryType::new(header_entry_def_id, header_zome_id, entry_visibility);

    let original_entry_address =
        get_original_address(call_context.clone(), original_header_hash.clone())?;

    // build a header for the entry being updated
    let header_builder = builder::EntryUpdate {
        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry_hash,
        original_header_address: original_header_hash,
        original_entry_address,
    };
    let call =
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
            async move {
                let source_chain = &mut workspace.source_chain;
                // push the header and the entry into the source chain
                let header_hash = source_chain.put(header_builder, Some(entry)).await?;
                // fetch the element we just added so we can integrate its DhtOps
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
    let header_address =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        }))???;

    // return the hash of the updated entry
    // note that validation is handled by the workflow
    // if the validation fails this update will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(UpdateEntryOutput::new(header_address))
}

// relying on tests for get_details
