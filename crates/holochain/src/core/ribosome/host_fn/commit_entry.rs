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
#[allow(clippy::extra_unused_lifetimes)]
pub fn commit_entry<'a>(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    // destructure the args out into an app type def id and entry
    let (entry_def_id, entry) = input.into_inner();

    // build the entry hash
    let async_entry = entry.clone();
    let entry_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        holochain_types::entry::EntryHashed::with_data(async_entry).await
    })?
    .into_hash();

    // extract the zome position
    let header_zome_id: holochain_types::header::ZomeId = match ribosome
        .dna_file
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &host_context.zome_name)
    {
        Some(index) => holochain_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(host_context.zome_name.clone()))?,
    };

    // extract the entry defs for a zome
    let (header_entry_def_id, entry_visibility) = match match ribosome
        .run_entry_defs(host_context.workspace.clone(), EntryDefsInvocation)?
    {
        // the ribosome returned some defs
        EntryDefsResult::Defs(defs) => {
            let maybe_entry_defs = defs.get(&host_context.zome_name);
            match maybe_entry_defs {
                // convert the entry def id string into a numeric position in the defs
                Some(entry_defs) => match entry_defs.entry_def_id_position(entry_def_id.clone()) {
                    // build an app entry type from the entry def at the found position
                    Some(index) => Some((
                        holochain_types::header::EntryDefId::from(index as u8),
                        entry_defs[index].visibility,
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

    let app_entry_type = AppEntryType::new(header_entry_def_id, header_zome_id, entry_visibility);

    // build a header for the entry being committed
    let header_builder = builder::EntryCreate {
        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry_hash.clone(),
    };
    let call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderAddress>> {
        async move {
            let source_chain = &mut workspace.source_chain;
            // push the header and the entry into the source chain
            source_chain.put(header_builder, Some(entry)).await
        }
        .boxed()
    };
    tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
        unsafe { host_context.workspace.apply_mut(call).await }
    }))???;

    // return the hash of the committed entry
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(CommitEntryOutput::new(entry_hash.into()))
}

#[cfg(test)]
pub mod wasm_test {
    use super::commit_entry;
    use crate::core::ribosome::error::RibosomeError;
    use crate::core::ribosome::HostContextFixturator;
    use crate::core::state::source_chain::ChainInvalidReason;
    use crate::core::{
        queue_consumer::TriggerSender,
        state::source_chain::SourceChainError,
        workflow::{
            integrate_dht_ops_workflow::{integrate_dht_ops_workflow, IntegrateDhtOpsWorkspace},
            produce_dht_ops_workflow::{produce_dht_ops_workflow, ProduceDhtOpsWorkspace},
        },
    };
    use crate::fixt::EntryFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use holo_hash::Hashable;
    use holo_hash::Hashed;
    use holo_hash_core::HoloHashCoreHash;
    use holochain_types::fixt::AppEntry;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::entry_def::EntryDefId;
    use holochain_zome_types::CommitEntryInput;
    use holochain_zome_types::CommitEntryOutput;
    use holochain_zome_types::GetEntryOutput;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    /// we cannot commit before genesis
    async fn commit_pre_genesis_test() {
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let ribosome =
            WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::CommitEntry]))
                .next()
                .unwrap();
        let mut host_context = HostContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        host_context.zome_name = TestWasm::CommitEntry.into();
        host_context.workspace = raw_workspace;
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::from("post");
        let input = CommitEntryInput::new((entry_def_id, app_entry.clone()));

        let output = commit_entry(Arc::new(ribosome), Arc::new(host_context), input);

        assert_eq!(
            format!("{:?}", output.unwrap_err()),
            format!(
                "{:?}",
                RibosomeError::SourceChainError(SourceChainError::InvalidStructure(
                    ChainInvalidReason::GenesisDataMissing
                ))
            ),
        );
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn directly
    async fn commit_entry_test() {
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let ribosome =
            WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::CommitEntry]))
                .next()
                .unwrap();
        let mut host_context = HostContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        host_context.zome_name = TestWasm::CommitEntry.into();
        host_context.workspace = raw_workspace;
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::from("post");
        let input = CommitEntryInput::new((entry_def_id, app_entry.clone()));

        let output = commit_entry(Arc::new(ribosome), Arc::new(host_context), input).unwrap();

        let app_entry_hash = holochain_types::entry::EntryHashed::with_data(app_entry.clone())
            .await
            .unwrap()
            .into_hash();

        // this should be the hash of the newly committed entry
        assert_eq!(app_entry_hash.get_raw(), output.into_inner().get_raw(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_commit_entry_test() {
        holochain_types::observability::test_run().ok();
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let output = {
            let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
            let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

            // commits fail validation if we don't do genesis
            crate::core::workflow::fake_genesis(&mut workspace.source_chain)
                .await
                .unwrap();

            // get the result of a commit entry
            let output: CommitEntryOutput = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(raw_workspace, TestWasm::CommitEntry, "commit_entry", ())
            };
            holochain_state::env::WriteManager::with_commit(&env_ref, |writer| {
                crate::core::state::workspace::Workspace::flush_to_txn(workspace, writer)
            })
            .unwrap();
            output
        };

        // this should be the hash of the newly committed entry
        assert_eq!(
            vec![
                62, 54, 23, 199, 14, 51, 180, 172, 119, 192, 27, 49, 206, 111, 170, 221, 23, 232,
                203, 86, 215, 89, 178, 16, 162, 24, 159, 168, 45, 255, 28, 217, 94, 223, 228, 142
            ]
            .as_slice(),
            output.into_inner().get_raw(),
        );

        // Needs metadata to return get
        {
            use crate::core::state::workspace::Workspace;
            use holochain_state::env::ReadManager;

            let (mut qt, mut rx) = TriggerSender::new();
            {
                let reader = env_ref.reader().unwrap();
                let workspace = ProduceDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                produce_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                rx.listen().await.unwrap();
            }
            {
                let reader = env_ref.reader().unwrap();
                let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                integrate_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                rx.listen().await.unwrap();
            }
        }

        let round: GetEntryOutput = {
            let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
            let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();
            let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
            crate::call_test_ribosome!(raw_workspace, TestWasm::CommitEntry, "get_entry", ())
        };

        let sb = match round.into_inner() {
            Some(holochain_zome_types::entry::Entry::App(serialized_bytes)) => serialized_bytes,
            other => panic!(format!("unexpected output: {:?}", other)),
        };
        // this should be the content "foo" of the committed post
        assert_eq!(&vec![163, 102, 111, 111], sb.bytes(),)
    }
}
