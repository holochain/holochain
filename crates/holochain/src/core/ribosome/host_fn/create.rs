use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;

use holochain_types::prelude::*;
use std::sync::Arc;

/// create element
#[allow(clippy::extra_unused_lifetimes)]
pub fn create<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CreateInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let CreateInput {
                entry_location,
                entry,
                chain_top_ordering,
            } = input;

            // TODO: This can be removed when we remove zome ids from headers.
            let zome_id = match &entry_location {
                EntryDefLocation::App(entry_index) => {
                    match ribosome.zome_types().find_zome_id_from_entry(&entry_index) {
                        Some(i) => Some(i),
                        None => {
                            return Err(WasmError::Host(format!(
                                "Entry index {} not found in DNA {}",
                                entry_index.0,
                                ribosome.dna_hash()
                            )))
                        }
                    }
                }
                EntryDefLocation::CapClaim | EntryDefLocation::CapGrant => None,
            };

            // Countersigned entries have different header handling.
            match entry {
                Entry::CounterSign(_, _) => tokio_helper::block_forever_on(async move {
                    call_context
                        .host_context
                        .workspace_write()
                        .source_chain()
                        .as_ref()
                        .expect("Must have source chain if write_workspace access is given")
                        .put_countersigned(None, entry, chain_top_ordering)
                        .await
                        .map_err(|source_chain_error| {
                            WasmError::Host(source_chain_error.to_string())
                        })
                }),
                _ => {
                    // build the entry hash
                    let entry_hash = EntryHash::with_data_sync(&entry);

                    // extract the entry defs for a zome
                    let entry_type = match entry_location {
                        EntryDefLocation::App(entry_def_index) => {
                            let integrity_zomes = &ribosome.dna_def().content.integrity_zomes;
                            let header_zome_id = zome_id
                                .expect("Zome can never be none for a EntryDefLocation::App");
                            let (zome_name, zome) = integrity_zomes.get(header_zome_id.0 as usize)
                                    .ok_or_else(|| WasmError::Guest(format!("Tried to get zome id {} but there are only {} integrity zomes", header_zome_id.0, integrity_zomes.len())))?;
                            let entry_visibility = get_entry_visibility(
                                call_context.as_ref(),
                                zome_name,
                                zome.clone(),
                                entry_def_index,
                            )?;
                            let app_entry_type = AppEntryType::new(
                                entry_def_index,
                                header_zome_id,
                                entry_visibility,
                            );
                            EntryType::App(app_entry_type)
                        }
                        EntryDefLocation::CapGrant => EntryType::CapGrant,
                        EntryDefLocation::CapClaim => EntryType::CapClaim,
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
                    tokio_helper::block_forever_on(async move {
                        // push the header and the entry into the source chain
                        call_context
                            .host_context
                            .workspace_write()
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain if write_workspace access is given")
                            .put(None, header_builder, Some(entry), chain_top_ordering)
                            .await
                            .map_err(|source_chain_error| {
                                WasmError::Host(source_chain_error.to_string())
                            })
                    })
                }
            }
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create".into(),
            )
            .to_string(),
        )),
    }
}

fn get_entry_visibility(
    call_context: &CallContext,
    zome_name: &ZomeName,
    zome: IntegrityZomeDef,
    entry_def_position: EntryDefIndex,
) -> Result<EntryVisibility, WasmError> {
    let key = EntryDefBufferKey {
        zome,
        entry_def_position,
    };
    call_context
        .host_context()
        .call_zome_handle()
        .get_entry_def(&key)
        .map(|e| e.visibility)
        .ok_or_else(|| {
            WasmError::Host(
                RibosomeError::EntryDefs(
                    zome_name.clone(),
                    format!(
                        "entry def not found for entry index {}",
                        entry_def_position.0
                    ),
                )
                .to_string(),
            )
        })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::create;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::fixt::*;
    use crate::sweettest::*;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holo_hash::AnyDhtHash;
    use holo_hash::EntryHash;
    use holochain_state::source_chain::SourceChainResult;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_wasm_test_utils::TestWasmPair;
    use observability;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn directly
    async fn create_entry_test() {
        let ribosome =
            RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Create]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.zome = TestWasmPair::<IntegrityZome, CoordinatorZome>::from(TestWasm::Create)
            .coordinator
            .erase_type();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let host_access_2 = host_access.clone();
        call_context.host_context = host_access.into();
        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let input = CreateInput::new(
            EntryDefLocation::app(0.into()),
            app_entry.clone(),
            ChainTopOrdering::default(),
        );

        let output = create(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            // The line below was added when migrating to rust edition 2021, per
            // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
            let _ = &host_access_2;
            SourceChainResult::Ok(
                host_access_2
                    .workspace
                    .source_chain()
                    .as_ref()
                    .unwrap()
                    .chain_head()?
                    .0,
            )
        })
        .unwrap();

        assert_eq!(chain_head, output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_create_entry_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Create).await;

        // get the result of a commit entry
        let _output: HeaderHash = conductor.call(&alice, "create_entry", ()).await;

        // entry should be gettable.
        let round: Option<Element> = conductor.call(&alice, "get_entry", ()).await;

        let round_twice: Vec<Option<Element>> = conductor.call(&alice, "get_entry_twice", ()).await;

        let bytes: Vec<u8> = match round.clone().and_then(|el| el.into()) {
            Some(holochain_zome_types::entry::Entry::App(entry_bytes)) => {
                entry_bytes.bytes().to_vec()
            }
            other => panic!("unexpected output: {:?}", other),
        };
        // this should be the content "foo" of the committed post
        assert_eq!(vec![163, 102, 111, 111], bytes);

        assert_eq!(round_twice, vec![round.clone(), round],);
    }

    #[tokio::test(flavor = "multi_thread")]
    // TODO: rewrite with sweettest and check if still flaky.
    // maackle: this consistently passes for me with n = 37
    //          but starts to randomly lock up at n = 38,
    //          and fails consistently for higher values
    async fn multiple_create_entry_limit_test() {
        const N: u32 = 50;

        observability::test_run().unwrap();
        let mut conductor = SweetConductor::from_standard_config().await;
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MultipleCalls])
            .await
            .unwrap();

        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let (cell,) = app.into_tuple();

        let _: () = conductor
            .call(
                &cell.zome(TestWasm::MultipleCalls),
                "create_entry_multiple",
                N,
            )
            .await;

        let output: holochain_zome_types::bytes::Bytes = conductor
            .call(&cell.zome(TestWasm::MultipleCalls), "get_entry_multiple", N)
            .await;

        let expected: Vec<u8> = (0..N).flat_map(|i| i.to_le_bytes()).collect();

        assert_eq!(output.into_vec(), expected);
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
