use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::host_fn::entry_hash::entry_hash;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holochain_types::composite_hash::HeaderAddress;
use holochain_types::header::builder;
use holochain_types::header::AppEntryType;
use holochain_types::header::EntryType;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use holochain_zome_types::EntryHashInput;
use std::convert::TryInto;
use std::sync::Arc;

pub async fn commit_entry<'a>(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    let (entry_def_id, entry) = input.into_inner();

    let entry_hash = entry_hash(
        Arc::clone(&ribosome),
        Arc::clone(&host_context),
        EntryHashInput::new(entry.clone()),
    )
    .await?
    .into_inner();

    let entry_defs =
        match ribosome.run_entry_defs(host_context.workspace.clone(), EntryDefsInvocation)? {
            EntryDefsResult::Defs(defs) => {
                let maybe_entry_defs = defs.get(&host_context.zome_name);
                match maybe_entry_defs {
                    Some(entry_defs) => entry_defs.to_owned(),
                    _ => Err(RibosomeError::EntryDefs(
                        host_context.zome_name.clone(),
                        "entry defs not found".to_string(),
                    ))?,
                }
            }
            _ => Err(RibosomeError::EntryDefs(
                host_context.zome_name.clone(),
                "entry defs not found".to_string(),
            ))?,
        };

    let app_entry_type = match entry_defs.entry_def_id_position(entry_def_id.clone()) {
        Some(index) => AppEntryType::new(vec![index as _], index as _, entry_defs[0].visibility),
        None => Err(RibosomeError::EntryDefs(
            host_context.zome_name.clone(),
            format!("entry def not found for {:?}", entry_def_id),
        ))?,
    };

    let header_builder = builder::EntryCreate {
        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry_hash.clone().try_into()?,
    };
    let call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderAddress>> {
        async move {
            let source_chain = &mut workspace.source_chain;
            source_chain.put(header_builder, Some(entry)).await
        }
        .boxed()
    };
    let _result = unsafe { host_context.workspace.apply_mut(call).await? };

    Ok(CommitEntryOutput::new(entry_hash.into()))
}
