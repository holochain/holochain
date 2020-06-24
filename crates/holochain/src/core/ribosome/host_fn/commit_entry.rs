use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::Hashable;
use holo_hash::Hashed;
use holochain_types::composite_hash::HeaderAddress;
use holochain_types::header::builder;
use holochain_types::header::AppEntryType;
use holochain_types::header::EntryType;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;

/// commit an entry
pub async fn commit_entry<'a>(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    dbg!("xxx");
    // destructure the args out into an app type def id and entry
    let (entry_def_id, entry) = input.into_inner();
    dbg!("xxx");

    // build the entry hash
    let entry_hash = holochain_types::entry::EntryHashed::with_data(entry.clone())
        .await?
        .into_hash();
    dbg!("xxx");

    // extract the entry defs for a zome
    let app_entry_type = match match ribosome
        .run_entry_defs(host_context.workspace.clone(), EntryDefsInvocation)?
    {
        // the ribosome returned some defs
        EntryDefsResult::Defs(defs) => {
            let maybe_entry_defs = defs.get(&host_context.zome_name);
            match maybe_entry_defs {
                // convert the entry def id string into a numeric position in the defs
                Some(entry_defs) => match entry_defs.entry_def_id_position(entry_def_id.clone()) {
                    // build an app entry type from the entry def at the found position
                    Some(index) => Some(AppEntryType::new(
                        vec![index as _],
                        index as _,
                        entry_defs[0].visibility,
                    )),
                    None => None,
                },
                None => None,
            }
        }
        _ => None,
    } {
        Some(app_entry_type) => app_entry_type,
        None => Err(RibosomeError::EntryDefs(
            host_context.zome_name.clone(),
            format!("entry def not found for {:?}", entry_def_id),
        ))?,
    };
    dbg!("xxx");

    // build a header for the entry being committed
    let header_builder = builder::EntryCreate {
        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry_hash.clone(),
    };
    dbg!("xxx");
    let call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderAddress>> {
        async move {
            let source_chain = &mut workspace.source_chain;
            // push the header and the entry into the source chain
            source_chain.put(header_builder, Some(entry)).await
        }
        .boxed()
    };
    dbg!("xxx");
    unsafe { host_context.workspace.apply_mut(call).await }??;

    dbg!("xxx");
    // return the hash of the committed entry
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(CommitEntryOutput::new(entry_hash.into()))
}

#[cfg(test)]
pub mod wasm_test {
    // use crate::core::ribosome::HostContextFixturator;
    use crate::fixt::EntryDefIdFixturator;
    use crate::fixt::EntryFixturator;
    // use crate::fixt::WasmRibosomeFixturator;
    use holochain_zome_types::CommitEntryInput;
    use holochain_zome_types::CommitEntryOutput;
    use holochain_wasm_test_utils::TestWasm;
    // use super::commit_entry;
    // use std::sync::Arc;

    // #[tokio::test(threaded_scheduler)]
    // /// we can get an entry hash out of the fn directly
    // async fn commit_entry_test() {
    //     let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
    //         .next()
    //         .unwrap();
    //     let host_context = HostContextFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let app_entry = EntryFixturator::new(crate::fixt::curve::AppEntry)
    //         .next()
    //         .unwrap();
    //     let entry_def_id = EntryDefIdFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let input = CommitEntryInput::new((entry_def_id, app_entry));
    //
    //     let output: CommitEntryOutput = tokio::task::spawn(async move {
    //         commit_entry(Arc::new(ribosome), Arc::new(host_context), input)
    //             .await
    //             .unwrap()
    //     })
    //     .await
    //     .unwrap();
    //
    //     // assert_eq!(DebugOutput::new(()), output);
    //     println!("{:?}", output);
    // }

    #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    async fn ribosome_commit_entry_test() {
        let app_entry = EntryFixturator::new(crate::fixt::curve::AppEntry).next().unwrap();
        let entry_def_id = EntryDefIdFixturator::new(fixt::Unpredictable).next().unwrap();
        let input = CommitEntryInput::new((entry_def_id, app_entry));
        let output: CommitEntryOutput = crate::call_test_ribosome!(
            TestWasm::CommitEntry,
            "commit_entry",
            input
        );
        println!("{:?}", output);
        // assert_eq!(output, DebugOutput::new(()));
    }
}
