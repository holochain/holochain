use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_authored;
use holochain_wasmer_host::prelude::WasmError;

use holo_hash::HasHash;
use holochain_types::prelude::*;
use std::sync::Arc;

/// create element
#[allow(clippy::extra_unused_lifetimes)]
pub fn create<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: EntryWithDefId,
) -> Result<HeaderHash, WasmError> {
    // build the entry hash
    let async_entry = AsRef::<Entry>::as_ref(&input).to_owned();
    let entry_hash =
        holochain_types::entry::EntryHashed::from_content_sync(async_entry).into_hash();

    // extract the zome position
    let header_zome_id = ribosome
        .zome_to_id(&call_context.zome)
        .expect("Failed to get ID for current zome");

    // extract the entry defs for a zome
    let entry_type = match AsRef::<EntryDefId>::as_ref(&input) {
        EntryDefId::App(entry_def_id) => {
            let (header_entry_def_id, entry_visibility) = extract_entry_def(
                ribosome,
                call_context.clone(),
                entry_def_id.to_owned().into(),
            )?;
            let app_entry_type =
                AppEntryType::new(header_entry_def_id, header_zome_id, entry_visibility);
            EntryType::App(app_entry_type)
        }
        EntryDefId::CapGrant => EntryType::CapGrant,
        EntryDefId::CapClaim => EntryType::CapClaim,
    };

    // build a header for the entry being committed
    let header_builder = builder::Create {
        entry_type,
        entry_hash,
    };

    // return the hash of the committed entry
    // note that validation is handled by the workflow
    // if the validation fails this commit will be rolled back by virtue of the DB transaction
    // being atomic
    let entry = AsRef::<Entry>::as_ref(&input).to_owned();
    tokio_helper::block_forever_on(async move {
        let mut guard = call_context.host_access.workspace().write().await;
        let workspace: &mut CallZomeWorkspace = &mut guard;
        let source_chain = &mut workspace.source_chain;
        // push the header and the entry into the source chain
        let header_hash = source_chain
            .put(header_builder, Some(entry))
            .await
            .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
        // fetch the element we just added so we can integrate its DhtOps
        let element = source_chain
            .get_element(&header_hash)
            .map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?
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

pub fn extract_entry_def(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    entry_def_id: EntryDefId,
) -> Result<(holochain_zome_types::header::EntryDefIndex, EntryVisibility), WasmError> {
    let app_entry_type = match ribosome
        .run_entry_defs((&call_context.host_access).into(), EntryDefsInvocation)
        .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?
    {
        // the ribosome returned some defs
        EntryDefsResult::Defs(defs) => {
            let maybe_entry_defs = defs.get(call_context.zome.zome_name());
            match maybe_entry_defs {
                // convert the entry def id string into a numeric position in the defs
                Some(entry_defs) => {
                    match entry_defs.entry_def_index_from_id(entry_def_id.clone()) {
                        // build an app entry type from the entry def at the found position
                        Some(index) => Some((index, entry_defs[index.0 as usize].visibility)),
                        None => None,
                    }
                }
                None => None,
            }
        }
        _ => None,
    };
    match app_entry_type {
        Some(app_entry_type) => Ok(app_entry_type),
        None => Err(WasmError::Host(
            RibosomeError::EntryDefs(
                call_context.zome.zome_name().clone(),
                format!("entry def not found for {:?}", entry_def_id),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::create;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::*;
    use crate::test_utils::setup_app;
    use crate::{conductor::api::ZomeCall, core::ribosome::error::RibosomeError};
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holo_hash::AnyDhtHash;
    use holo_hash::EntryHash;
    use holochain_state::source_chain::ChainInvalidReason;
    use holochain_state::source_chain::SourceChainError;
    use holochain_state::source_chain::SourceChainResult;
    use holochain_types::app::InstalledCell;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_types::fixt::AppEntry;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;
    use observability;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    /// we cannot commit before genesis
    async fn create_pre_genesis_test() {
        // test workspace boilerplate
        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let ribosome =
            RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Create]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.zome = TestWasm::Create.into();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        call_context.host_access = host_access.into();
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::App("post".into());
        let input = EntryWithDefId::new(entry_def_id, app_entry.clone());

        let output = create(Arc::new(ribosome), Arc::new(call_context), input);

        assert_eq!(
            format!("{}", output.unwrap_err().to_string()),
            format!(
                "{}",
                WasmError::Host(
                    RibosomeError::SourceChainError(SourceChainError::InvalidStructure(
                        ChainInvalidReason::GenesisDataMissing
                    ))
                    .to_string()
                )
            ),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn directly
    async fn create_entry_test<'a>() {
        // test workspace boilerplate
        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let ribosome =
            RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Create]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.zome = TestWasm::Create.into();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();
        call_context.host_access = host_access.into();
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let entry_def_id = EntryDefId::App("post".into());
        let input = EntryWithDefId::new(entry_def_id, app_entry.clone());

        let output = create(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            SourceChainResult::Ok(
                workspace_lock
                    .read()
                    .await
                    .source_chain
                    .chain_head()?
                    .to_owned(),
            )
        })
        .unwrap();

        assert_eq!(chain_head, output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_create_entry_test<'a>() {
        observability::test_run().ok();
        // test workspace boilerplate
        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        // get the result of a commit entry
        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Create, "create_entry", ());

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            SourceChainResult::Ok(
                workspace_lock
                    .read()
                    .await
                    .source_chain
                    .chain_head()?
                    .to_owned(),
            )
        })
        .unwrap();

        assert_eq!(&chain_head, &output);

        let round: Option<Element> =
            crate::call_test_ribosome!(host_access, TestWasm::Create, "get_entry", ());

        let bytes: Vec<u8> = match round.and_then(|el| el.into()) {
            Some(holochain_zome_types::entry::Entry::App(entry_bytes)) => {
                entry_bytes.bytes().to_vec()
            }
            other => panic!(format!("unexpected output: {:?}", other)),
        };
        // this should be the content "foo" of the committed post
        assert_eq!(vec![163, 102, 111, 111], bytes);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "david.b (this test is flaky)"]
    // maackle: this consistently passes for me with n = 37
    //          but starts to randomly lock up at n = 38,
    //          and fails consistently for higher values
    async fn multiple_create_entry_limit_test() {
        observability::test_run().unwrap();
        let dna_file = DnaFile::new(
            DnaDef {
                name: "create_multi_test".to_string(),
                uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                zomes: vec![TestWasm::MultipleCalls.into()].into(),
            },
            vec![TestWasm::MultipleCalls.into()],
        )
        .await
        .unwrap();

        // //////////
        // END DNA
        // //////////

        // ///////////
        // START ALICE
        // ///////////

        let alice_agent_id = fake_agent_pubkey_1();
        let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
        let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

        // /////////
        // END ALICE
        // /////////

        // /////////
        // START BOB
        // /////////

        let bob_agent_id = fake_agent_pubkey_2();
        let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
        let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

        // ///////
        // END BOB
        // ///////

        // ///////////////
        // START CONDUCTOR
        // ///////////////

        let (_tmpdir, _app_api, handle) = setup_app(
            vec![(
                "APPropriated",
                vec![(alice_installed_cell, None), (bob_installed_cell, None)],
            )],
            vec![dna_file.clone()],
        )
        .await;

        // /////////////
        // END CONDUCTOR
        // /////////////

        // ALICE DOING A CALL

        let n = 50_u32;

        // alice create a bunch of entries
        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::MultipleCalls.into(),
                cap: None,
                fn_name: "create_entry_multiple".into(),
                payload: ExternIO::encode(n).unwrap(),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(output, ZomeCallResponse::Ok(ExternIO::encode(()).unwrap()));

        // bob get the entries
        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id,
                zome_name: TestWasm::MultipleCalls.into(),
                cap: None,
                fn_name: "get_entry_multiple".into(),
                payload: ExternIO::encode(n).unwrap(),
                provenance: alice_agent_id,
            })
            .await
            .unwrap()
            .unwrap();

        // check the vals
        let mut expected = vec![];
        for i in 0..n {
            expected.append(&mut i.to_le_bytes().to_vec());
        }
        assert_eq!(
            output,
            ZomeCallResponse::Ok(ExternIO::encode(expected).unwrap())
        );

        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_serialize_bytes_hash() {
        observability::test_run().ok();
        #[derive(Default, SerializedBytes, Serialize, Deserialize, Debug)]
        #[repr(transparent)]
        #[serde(transparent)]
        struct Post(String);
        impl TryFrom<&Post> for Entry {
            type Error = EntryError;
            fn try_from(post: &Post) -> Result<Self, Self::Error> {
                Entry::app(post.try_into()?)
            }
        }

        // This is normal trip that works as expected
        let entry: Entry = (&Post("foo".into())).try_into().unwrap();
        let entry_hash = EntryHash::with_data_sync(&entry);
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
    }
}
