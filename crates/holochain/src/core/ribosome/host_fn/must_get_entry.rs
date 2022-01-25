use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::event::GetRequest;
use crate::core::ribosome::RibosomeError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetEntryInput,
) -> Result<EntryHashed, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ read_workspace_deterministic: Permission::Allow, .. } => {
            let entry_hash = input.into_inner();
            let network = call_context.host_context.network().clone();
            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = Cascade::from_workspace_network(&workspace, network);
                match cascade
                    .retrieve_entry(entry_hash.clone(),
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
                    })
                    .await
                    .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                        Some(entry) => Ok(entry),
                        None => match call_context.host_context {
                            HostContext::EntryDefs(_) | HostContext::GenesisSelfCheck(_) | HostContext::MigrateAgent(_) | HostContext::PostCommit(_) | HostContext::ZomeCall(_) => Err(WasmError::Host(format!("Failed to get EntryHashed {}", entry_hash))),
                            HostContext::Init(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(vec![entry_hash.into()]))?
                                )?
                            ))),
                            HostContext::ValidateCreateLink(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(ValidateLinkCallbackResult::UnresolvedDependencies(vec![entry_hash.into()]))?
                                )?
                            ))),
                            HostContext::Validate(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(&ExternIO::encode(&ValidateCallbackResult::UnresolvedDependencies(vec![entry_hash.into()]))?)?
                            ))),
                            HostContext::ValidationPackage(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&
                                ExternIO::encode(ValidationPackageCallbackResult::UnresolvedDependencies(vec![entry_hash.into()]))?
                            )?))),
                        },
                    }
            })
        },
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "must_get_entry".into()
        ).to_string()))
    }

}

// #[cfg(test)]
// pub mod test {
//     use ::fixt::prelude::fixt;
//     use hdk::prelude::*;
//     use holochain_wasm_test_utils::TestWasm;
//     use crate::fixt::ZomeCallHostAccessFixturator;
//     use crate::core::ribosome::RibosomeError;
//     use std::sync::Arc;
//     use crate::fixt::ValidateHostAccessFixturator;
//     use crate::fixt::curve::Zomes;
//     use crate::fixt::RealRibosomeFixturator;
//     use crate::fixt::ValidateInvocationFixturator;
//     use crate::core::ribosome::ZomesToInvoke;
//     use crate::core::ribosome::RibosomeT;
//     use crate::core::ribosome::guest_callback::validate::ValidateResult;
//     use ::fixt::prelude::*;

//     /// Mimics inside the must_get wasm.
//     #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
//     struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

//     #[tokio::test(flavor = "multi_thread")]
//     async fn ribosome_must_get_entry_test<'a>() {
//         observability::test_run().ok();
//         let author = fixt!(AgentPubKey, Predictable, 0);
//         let mut host_access = fixt!(ZomeCallHostAccess, Predictable);

//         // get the result of a commit entry
//         let (header_hash, header_reference_hash, element_reference_hash, entry_reference_hash): (HeaderHash, HeaderHash, HeaderHash, HeaderHash) =
//             crate::call_test_ribosome!(host_access, TestWasm::MustGet, "create_entry", ()).unwrap();

//         let round_element: Element =
//             crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", header_hash).unwrap();

//         let header_reference_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", header_reference_hash).unwrap();
//         let element_reference_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", element_reference_hash).unwrap();
//         let entry_reference_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", entry_reference_hash).unwrap();

//         let (header_dangling_header_hash, element_dangling_header_hash, entry_dangling_header_hash): (HeaderHash, HeaderHash, HeaderHash) = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "create_dangling_references", ()).unwrap();
//         let header_dangling_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", header_dangling_header_hash).unwrap();
//         let element_dangling_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", element_dangling_header_hash).unwrap();
//         let entry_dangling_element: Element = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", entry_dangling_header_hash).unwrap();

//         let round_entry = round_element.entry().to_app_option::<Something>().unwrap().unwrap();

//         assert_eq!(
//             &round_entry,
//             &Something(vec![1, 2, 3])
//         );

//         let fail_header_hash = HeaderHash::from_raw_32([0; 32].to_vec());

//         let element_fail: Result<Element, RibosomeError> = crate::call_test_ribosome!(
//             host_access,
//             TestWasm::MustGet,
//             "must_get_valid_element",
//             fail_header_hash
//         );

//         match element_fail {
//             Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
//                 "Failed to get Element uhCkkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
//                 e,
//             ),
//             _ => unreachable!(),
//         };

//         let signed_header: SignedHeaderHashed = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_header", header_hash).unwrap();

//         assert_eq!(
//             &signed_header,
//             round_element.signed_header(),
//         );

//         let header_fail: Result<SignedHeaderHashed, RibosomeError> = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_header", fail_header_hash);

//         match header_fail {
//             Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
//                 "Failed to get SignedHeaderHashed uhCkkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
//                 e,
//             ),
//             _ => unreachable!(),
//         }

//         let entry_hash = match signed_header.header() {
//             Header::Create(create) => create.entry_hash.clone(),
//             _ => unreachable!(),
//         };

//         let entry: EntryHashed = crate::call_test_ribosome!(
//             host_access,
//             TestWasm::MustGet,
//             "must_get_entry",
//             entry_hash
//         ).unwrap();

//         assert_eq!(
//             &ElementEntry::Present(entry.as_content().clone()),
//             round_element.entry(),
//         );

//         let fail_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

//         let entry_fail: Result<EntryHashed, RibosomeError> = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_entry", fail_entry_hash);

//         match entry_fail {
//             Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
//                 "Failed to get EntryHashed uhCEkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
//                 e,
//             ),
//             _ => unreachable!(),
//         }

//         let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::MustGet])).next().unwrap();
//         let test_network = crate::test_utils::test_network(
//             Some(ribosome.dna_def().as_hash().clone()),
//             Some(author),
//         )
//         .await;
//         let dna_network = test_network.dna_network();
//         host_access.network = dna_network;

//         let mut validate_invocation = fixt!(ValidateInvocation);
//         validate_invocation.element = Arc::new(header_reference_element.clone());
//         validate_invocation.zomes_to_invoke = ZomesToInvoke::One(TestWasm::MustGet.into());
//         validate_invocation.entry_def_id = Some(EntryDefId::App("header_reference".into()));
//         let mut validate_host_access = fixt!(ValidateHostAccess);
//         validate_host_access.network = host_access.network.clone();
//         validate_host_access.workspace = host_access.workspace.clone().into();

//         let header_reference_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(ValidateResult::Valid, header_reference_validate_result);

//         validate_invocation.element = Arc::new(element_reference_element.clone());
//         validate_invocation.entry_def_id = Some(EntryDefId::App("element_reference".into()));

//         let element_reference_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(ValidateResult::Valid, element_reference_validate_result);

//         validate_invocation.element = Arc::new(entry_reference_element.clone());
//         validate_invocation.entry_def_id = Some(EntryDefId::App("entry_reference".into()));

//         let entry_reference_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(ValidateResult::Valid, entry_reference_validate_result);

//         validate_invocation.element = Arc::new(header_dangling_element.clone());
//         validate_invocation.entry_def_id = Some(EntryDefId::App("header_reference".into()));

//         let header_dangling_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(
//             ValidateResult::UnresolvedDependencies(vec![fail_header_hash.clone().into()]),
//             header_dangling_validate_result,
//         );

//         validate_invocation.element = Arc::new(element_dangling_element);
//         validate_invocation.entry_def_id = Some(EntryDefId::App("element_reference".into()));

//         let element_dangling_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(
//             ValidateResult::UnresolvedDependencies(vec![fail_header_hash.clone().into()]),
//             element_dangling_validate_result,
//         );

//         validate_invocation.element = Arc::new(entry_dangling_element);
//         validate_invocation.entry_def_id = Some(EntryDefId::App("entry_reference".into()));

//         let entry_dangling_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone()).unwrap();

//         assert_eq!(
//             ValidateResult::UnresolvedDependencies(vec![fail_entry_hash.clone().into()]),
//             entry_dangling_validate_result,
//         );

//         // A garbage entry should fail to deserialize and return Ok(ValidateCallbackResult::Invalid) not Err(WasmError).
//         let garbage_entry = fixt!(Entry, Predictable, 1);
//         let mut garbage_element: Element = validate_invocation.element.as_ref().clone();
//         *garbage_element.as_entry_mut() = ElementEntry::Present(garbage_entry);


//         validate_invocation.element = Arc::new(garbage_element);

//         let garbage_entry_validate_result = ribosome.run_validate(validate_host_access.clone(), validate_invocation.clone());

//         assert_eq!(
//             garbage_entry_validate_result.unwrap(),
//             ValidateResult::Invalid("Serialize(Deserialize(\"invalid type: boolean `false`, expected a HoloHash of primitive hash_type\"))".into())
//         );
//     }
// }