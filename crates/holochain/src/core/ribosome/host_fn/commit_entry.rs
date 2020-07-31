use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::{
    call_zome_workflow::CallZomeWorkspace, integrate_dht_ops_workflow::integrate_to_cache,
};
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::{HasHash, HeaderHash};
use holochain_zome_types::header::builder;
use holochain_zome_types::header::AppEntryType;
use holochain_zome_types::header::EntryType;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;

/// commit an entry
#[allow(clippy::extra_unused_lifetimes)]
pub fn commit_entry<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    // destructure the args out into an app type def id and entry
    let (entry_def_id, entry) = input.into_inner();

    // build the entry hash
    let async_entry = entry.clone();
    let entry_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        holochain_types::entry::EntryHashed::from_content(async_entry).await
    })
    .into_hash();

    // extract the zome position
    let header_zome_id: holochain_zome_types::header::ZomeId = match ribosome
        .dna_file()
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &call_context.zome_name)
    {
        Some(index) => holochain_zome_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(call_context.zome_name.clone()))?,
    };

    // extract the entry defs for a zome
    let (header_entry_def_id, entry_visibility) = match match ribosome
        .run_entry_defs((&call_context.host_access).into(), EntryDefsInvocation)?
    {
        // the ribosome returned some defs
        EntryDefsResult::Defs(defs) => {
            let maybe_entry_defs = defs.get(&call_context.zome_name);
            match maybe_entry_defs {
                // convert the entry def id string into a numeric position in the defs
                Some(entry_defs) => match entry_defs.entry_def_id_position(entry_def_id.clone()) {
                    // build an app entry type from the entry def at the found position
                    Some(index) => Some((
                        holochain_zome_types::header::EntryDefId::from(index as u8),
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
            call_context.zome_name.clone(),
            format!("entry def not found for {:?}", entry_def_id),
        ))?,
    };

    let app_entry_type = AppEntryType::new(header_entry_def_id, header_zome_id, entry_visibility);

    // build a header for the entry being committed
    let header_builder = builder::EntryCreate {
        entry_type: EntryType::App(app_entry_type),
        entry_hash: entry_hash,
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
                    workspace.source_chain.cas(),
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

    // return the hash of the committed entry
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the lmdb transaction
    // being atomic
    Ok(CommitEntryOutput::new(header_address))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::commit_entry;
    use crate::core::ribosome::error::RibosomeError;
    use crate::core::state::source_chain::ChainInvalidReason;
    use crate::core::state::source_chain::SourceChainError;
    use crate::core::state::source_chain::SourceChainResult;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use futures::future::BoxFuture;
    use futures::future::FutureExt;
    use holo_hash::{AnyDhtHash, EntryHash, HeaderHash};
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::fixt::AppEntry;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::entry_def::EntryDefId;
    use holochain_zome_types::CommitEntryInput;
    use holochain_zome_types::CommitEntryOutput;
    use holochain_zome_types::Entry;
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
        let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let ribosome =
            WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::CommitEntry]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        call_context.zome_name = TestWasm::CommitEntry.into();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        call_context.host_access = host_access.into();
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::from("post");
        let input = CommitEntryInput::new((entry_def_id, app_entry.clone()));

        let output = commit_entry(Arc::new(ribosome), Arc::new(call_context), input);

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
    async fn commit_entry_test<'a>() {
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let ribosome =
            WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::CommitEntry]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        call_context.zome_name = TestWasm::CommitEntry.into();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace.clone();
        call_context.host_access = host_access.into();
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::from("post");
        let input = CommitEntryInput::new((entry_def_id, app_entry.clone()));

        let output = commit_entry(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        // the chain head should be the committed entry header
        let call =
            |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
                async move {
                    let source_chain = &mut workspace.source_chain;
                    Ok(source_chain.chain_head()?.to_owned())
                }
                .boxed()
            };
        let chain_head =
            tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
                unsafe { raw_workspace.apply_mut(call).await }
            }))
            .unwrap()
            .unwrap()
            .unwrap();

        assert_eq!(chain_head, output.into_inner(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_commit_entry_test<'a>() {
        holochain_types::observability::test_run().ok();
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace.clone();

        // get the result of a commit entry
        let output: CommitEntryOutput =
            crate::call_test_ribosome!(host_access, TestWasm::CommitEntry, "commit_entry", ());

        // the chain head should be the committed entry header
        let call =
            |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
                async move {
                    let source_chain = &mut workspace.source_chain;
                    Ok(source_chain.chain_head()?.to_owned())
                }
                .boxed()
            };
        let cloned_workspace = raw_workspace.clone();
        let chain_head =
            tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
                unsafe { cloned_workspace.apply_mut(call).await }
            }))
            .unwrap()
            .unwrap()
            .unwrap();

        assert_eq!(&chain_head, output.inner_ref());

        let round: GetEntryOutput =
            crate::call_test_ribosome!(host_access, TestWasm::CommitEntry, "get_entry", ());

        let sb = match round.into_inner().and_then(|el| el.into()) {
            Some(holochain_zome_types::entry::Entry::App(serialized_bytes)) => serialized_bytes,
            other => panic!(format!("unexpected output: {:?}", other)),
        };
        // this should be the content "foo" of the committed post
        assert_eq!(&vec![163, 102, 111, 111], sb.bytes(),)
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_serialize_bytes_hash() {
        holochain_types::observability::test_run().ok();
        #[derive(Default, SerializedBytes, Serialize, Deserialize)]
        #[repr(transparent)]
        #[serde(transparent)]
        struct Post(String);
        impl TryFrom<&Post> for Entry {
            type Error = SerializedBytesError;
            fn try_from(post: &Post) -> Result<Self, Self::Error> {
                Ok(Entry::App(post.try_into()?))
            }
        }

        // This is normal trip that works as expected
        let entry: Entry = (&Post("foo".into())).try_into().unwrap();
        let entry_hash = EntryHash::with_data(&entry).await;
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &entry_hash.to_string()
        );
        let sb: SerializedBytes = entry_hash.try_into().unwrap();
        let entry_hash: EntryHash = sb.try_into().unwrap();
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &entry_hash.to_string()
        );

        // Now I can convert to AnyDhtHash
        let any_hash: AnyDhtHash = entry_hash.clone().into();
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &entry_hash.to_string()
        );

        // The trip works as expected
        let sb: SerializedBytes = any_hash.try_into().unwrap();
        tracing::debug!(any_sb = ?sb);
        let any_hash: AnyDhtHash = sb.try_into().unwrap();
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &any_hash.to_string()
        );

        // Converting directly works
        let any_hash: AnyDhtHash = entry_hash.clone().try_into().unwrap();
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &any_hash.to_string()
        );

        // But if I go into SerializedBytes first from EntryHash
        let sb: SerializedBytes = entry_hash.clone().try_into().unwrap();
        tracing::debug!(entry_sb = ?sb);
        // Then back into AnyDhtHash
        // this changes the hash type to a header type?
        // "uhCkkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
        let any_hash: AnyDhtHash = sb.try_into().unwrap();
        assert_eq!(
            "uhCEkPjYXxw4ztKx3wBsxzm-q3Rfoy1bXWbIQohifqC3_HNle3-SO",
            &any_hash.to_string()
        );
        let sb: SerializedBytes = any_hash.clone().try_into().unwrap();
        tracing::debug!(any_2_sb = ?sb);
    }
}
