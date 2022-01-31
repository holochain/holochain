use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::event::GetRequest;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetEntryInput,
) -> Result<EntryHashed, WasmError> {
    dbg!(&input);
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let entry_hash = input.into_inner();
            let network = call_context.host_context.network().clone();
            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = Cascade::from_workspace_network(&workspace, network);
                match cascade
                    .retrieve_entry(
                        entry_hash.clone(),
                        // Set every GetOptions manually here.
                        // Using defaults is dangerous in a must_get as it can undermine determinism.
                        // We want refactors to explicitly consider this.
                        NetworkGetOptions {
                            remote_agent_count: None,
                            timeout_ms: None,
                            as_race: true,
                            race_timeout_ms: None,
                            // Never redirect as the returned entry must always match the hash.
                            follow_redirects: false,
                            all_live_headers_with_metadata: false,
                            // Redundant with retrieve_entry internals.
                            request_type: GetRequest::Pending,
                        },
                    )
                    .await
                    .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?
                {
                    Some(entry) => Ok(entry),
                    None => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheck(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(WasmError::Host(format!(
                            "Failed to get EntryHashed {}",
                            entry_hash
                        ))),
                        HostContext::Init(_) => RuntimeError::raise(Box::new(
                            WasmError::HostShortCircuit(holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    vec![entry_hash.into()],
                                ))?,
                            )?),
                        )),
                        HostContext::ValidateCreateLink(_) => {
                            RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(&ExternIO::encode(
                                    ValidateLinkCallbackResult::UnresolvedDependencies(vec![
                                        entry_hash.into(),
                                    ]),
                                )?)?,
                            )))
                        }
                        HostContext::Validate(_) => {
                            RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(&ExternIO::encode(
                                    &ValidateCallbackResult::UnresolvedDependencies(vec![
                                        entry_hash.into(),
                                    ]),
                                )?)?,
                            )))
                        }
                        HostContext::ValidationPackage(_) => {
                            RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(&ExternIO::encode(
                                    ValidationPackageCallbackResult::UnresolvedDependencies(vec![
                                        entry_hash.into(),
                                    ]),
                                )?)?,
                            )))
                        }
                    },
                }
            })
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_entry".into(),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
pub mod test {
    // use crate::core::ribosome::guest_callback::validate::ValidateResult;
    // use crate::core::ribosome::RibosomeError;
    // use crate::core::ribosome::RibosomeT;
    // use crate::core::ribosome::ZomesToInvoke;
    // use crate::fixt::curve::Zomes;
    // use crate::fixt::RealRibosomeFixturator;
    // use crate::fixt::ValidateHostAccessFixturator;
    // use crate::fixt::ValidateInvocationFixturator;
    // use ::fixt::prelude::fixt;
    // use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    // use std::sync::Arc;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::test_entry_impl;
    use unwrap_to::unwrap_to;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    #[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
    struct HeaderReference(HeaderHash);
    #[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
    struct EntryReference(EntryHash);
    #[derive(Deserialize, Serialize, SerializedBytes, Debug, Clone)]
    struct ElementReference(HeaderHash);

    test_entry_impl!(HeaderReference);
    test_entry_impl!(EntryReference);
    test_entry_impl!(ElementReference);

    const HEADER_REFERENCE_ENTRY_DEF_ID: &str = "header_reference";

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_entry_test<'a>() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, bob, alice_host_fn_caller, ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // // get the result of a commit entry
        // let (header_hash, header_reference_hash, element_reference_hash, entry_reference_hash): (
        //     HeaderHash,
        //     HeaderHash,
        //     HeaderHash,
        //     HeaderHash,
        // ) = conductor.call(&alice, "create_entry", ()).await;

        // let _round_element: Element = conductor
        //     .call(&alice, "must_get_valid_element", header_hash.clone())
        //     .await;

        // let _header_reference_element: Element = conductor
        //     .call(&alice, "must_get_valid_element", header_reference_hash)
        //     .await;
        // let _element_reference_element: Element = conductor
        //     .call(&alice, "must_get_valid_element", element_reference_hash)
        //     .await;
        // let _entry_reference_element: Element = conductor
        //     .call(&alice, "must_get_valid_element", entry_reference_hash)
        //     .await;

        let bad_header_hash = HeaderHash::from_raw_32(vec![0; 32]);
        // let bad_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

        let bad_header_reference = HeaderReference(bad_header_hash.clone());
        // let bad_element_reference = ElementReference(bad_header_hash);
        // let bad_entry_reference = EntryReference(bad_entry_hash);

        let header_dangling_header_hash = alice_host_fn_caller.commit_entry(Entry::try_from(bad_header_reference).unwrap(), HEADER_REFERENCE_ENTRY_DEF_ID).await;

        let must_get_header: Result<SignedHeaderHashed, _> = conductor.call_fallible(&alice, "must_get_header", header_dangling_header_hash.clone()).await;
        let must_get_valid_element: Result<Element, _> = conductor.call_fallible(&bob, "must_get_valid_element", header_dangling_header_hash).await;

        dbg!(&must_get_header);
        dbg!(&must_get_valid_element);

            // let entry = ThisWasmEntry::NeverValidates;
            // let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
            // let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
            // // 4
            // let invalid_header_hash = call_data
            //     .commit_entry(entry.clone().try_into().unwrap(), INVALID_ID)
            //     .await;

                // #[hdk_extern]
                // fn create_dangling_references(_: ()) -> ExternResult<(HeaderHash, HeaderHash, HeaderHash)> {
                //     let bad_header_hash = HeaderHash::from_raw_32(vec![0; 32]);
                //     let bad_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

                //     Ok((
                //         hdk::prelude::create_entry(HeaderReference(bad_header_hash.clone()))?,
                //         hdk::prelude::create_entry(ElementReference(bad_header_hash))?,
                //         hdk::prelude::create_entry(EntryReference(bad_entry_hash))?,
                //     ))
                // }

        // let (_header_dangling_header_hash, _element_dangling_header_hash, _entry_dangling_header_hash): (HeaderHash, HeaderHash, HeaderHash) = conductor.call(&alice, "create_dangling_references", ()).await;
        // let _header_dangling_element: Element = conductor
        //     .call(
        //         &alice,
        //         "must_get_valid_element",
        //         header_dangling_header_hash,
        //     )
        //     .await;
        // let _element_dangling_element: Element = conductor
        //     .call(
        //         &alice,
        //         "must_get_valid_element",
        //         element_dangling_header_hash,
        //     )
        //     .await;
        // let _entry_dangling_element: Element = conductor
        //     .call(&alice, "must_get_valid_element", entry_dangling_header_hash)
        //     .await;

        // let round_entry = round_element
        //     .entry()
        //     .to_app_option::<Something>()
        //     .unwrap()
        //     .unwrap();

        // assert_eq!(&round_entry, &Something(vec![1, 2, 3]));

        // let fail_header_hash = HeaderHash::from_raw_32([0; 32].to_vec());

        // let element_fail: Result<Element, _> = conductor
        //     .call_fallible(&alice, "must_get_valid_element", fail_header_hash.clone())
        //     .await;

        // assert_eq!(
        //     element_fail.unwrap_err().to_string(),
        //     "Failed to get Element uhCkkAAAAAAAAAAAAAAAAA3AAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
        // );

        // let signed_header: SignedHeaderHashed =
        //     conductor.call(&alice, "must_get_header", header_hash).await;

        // assert_eq!(&signed_header, round_element.signed_header(),);

        // let header_fail: Result<SignedHeaderHashed, _> = conductor
        //     .call_fallible(&alice, "must_get_header", fail_header_hash)
        //     .await;

        // assert_eq!(
        //     header_fail.unwrap_err().to_string(),
        //     "Failed to get SignedHeaderHashed uhCkkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
        // );

        // let entry_hash = match signed_header.header() {
        //     Header::Create(create) => create.entry_hash.clone(),
        //     _ => unreachable!(),
        // };

        // let entry: EntryHashed = conductor.call(&alice, "must_get_entry", entry_hash).await;

        // assert_eq!(
        //     &ElementEntry::Present(entry.as_content().clone()),
        //     round_element.entry(),
        // );

        // let fail_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

        // let entry_fail: Result<EntryHashed, _> = conductor
        //     .call_fallible(&alice, "must_get_entry", fail_entry_hash)
        //     .await;

        // assert_eq!(
        //     entry_fail.unwrap_err().to_string(),
        //     "Failed to get EntryHashed uhCEkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
        // );
    }
}
