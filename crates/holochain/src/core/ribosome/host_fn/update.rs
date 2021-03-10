use super::create::extract_entry_def;
use super::delete::get_original_address;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_authored;
use crate::core::workflow::CallZomeWorkspace;
use holochain_wasmer_host::prelude::WasmError;

use holo_hash::HasHash;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn update<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: UpdateInput,
) -> Result<HeaderHash, WasmError> {
    // destructure the args out into an app type def id and entry
    let UpdateInput { original_header_address, entry_with_def_id } = input;

    // build the entry hash
    let async_entry = AsRef::<Entry>::as_ref(&entry_with_def_id).to_owned();
    let entry_hash =
        holochain_types::entry::EntryHashed::from_content_sync(async_entry).into_hash();

    // extract the zome position
    let header_zome_id = ribosome.zome_to_id(&call_context.zome).map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;

    // extract the entry defs for a zome
    let entry_type = match AsRef::<EntryDefId>::as_ref(&entry_with_def_id) {
        EntryDefId::App(entry_def_id) => {
            let (header_entry_def_id, entry_visibility) =
                extract_entry_def(ribosome, call_context.clone(), entry_def_id.to_owned().into())?;
            let app_entry_type =
                AppEntryType::new(header_entry_def_id, header_zome_id, entry_visibility);
            EntryType::App(app_entry_type)
        }
        EntryDefId::CapGrant => EntryType::CapGrant,
        EntryDefId::CapClaim => EntryType::CapClaim,
    };

    let original_entry_address =
        get_original_address(call_context.clone(), original_header_address.clone())?;

    // build a header for the entry being updated.  Either the EntryWithDefId supplies a desired
    // Header timestamp, or we'll supply the current time when we put the Entry in the source_chain.
    let maybe_timestamp: Option<Timestamp> = AsRef::<Option<Timestamp>>::as_ref(&entry_with_def_id).to_owned();
    let header_builder = builder::Update {
        entry_type,
        entry_hash,
        original_header_address,
        original_entry_address,
    };

    let workspace_lock = call_context.host_access.workspace();

    // return the hash of the updated entry
    // note that validation is handled by the workflow
    // if the validation fails this update will be rolled back by virtue of the lmdb transaction
    // being atomic
    let entry = AsRef::<Entry>::as_ref(&entry_with_def_id).to_owned();
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let mut guard = workspace_lock.write().await;
        let workspace: &mut CallZomeWorkspace = &mut guard;
        let source_chain = &mut workspace.source_chain;
        // push the header and the entry into the source chain
        let header_hash = source_chain.put(header_builder, Some(entry), maybe_timestamp).await
	    .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
        // fetch the element we just added so we can integrate its DhtOps
        let element = source_chain
            .get_element(&header_hash).map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?
            .expect("Element we just put in SourceChain must be gettable");
        integrate_to_authored(
            &element,
            workspace.source_chain.elements(),
            &mut workspace.meta_authored,
        )
        .map_err(|dht_op_convert_error| WasmError::Host(dht_op_convert_error.to_string()))?;
        Ok(header_hash)
    })
}

// relying on tests for get_details
