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

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetEntryInput,
) -> Result<EntryHashed, WasmError> {
    let entry_hash = input.into_inner();
    let network = call_context.host_context.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_context.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        match cascade
            .retrieve_entry(entry_hash.clone(),
            // Set every GetOptions manually here.
            // Using defaults is dangerous as it can undermine determinism.
            // We want refactors to explicitly consider this.
            NetworkGetOptions {
                remote_agent_count: None,
                timeout_ms: None,
                as_race: true,
                race_timeout_ms: None,
                // Never redirect as the returned entry must always match the hash.
                follow_redirects: false,
                // Ignore deletes.
                all_live_headers_with_metadata: true,
                // Redundant with retrieve_entry internals.
                request_type: GetRequest::Pending,
            })
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                Some(entry) => Ok(entry),
                None => match call_context.host_context {
                    HostContext::EntryDefs(_) | HostContext::GenesisSelfCheck(_) | HostContext::MigrateAgent(_) | HostContext::PostCommit(_) | HostContext::ZomeCall(_) => Err(WasmError::Host(format!("Failed to get EntryHashed {}", entry_hash))),
                    HostContext::Init(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&Ok::<InitCallbackResult, ()>(InitCallbackResult::UnresolvedDependencies(vec![entry_hash.into()])))?))),
                    HostContext::ValidateCreateLink(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&Ok::<ValidateLinkCallbackResult, ()>(ValidateLinkCallbackResult::UnresolvedDependencies(vec![entry_hash.into()])))?))),
                    HostContext::Validate(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&Ok::<ValidateCallbackResult, ()>(ValidateCallbackResult::UnresolvedDependencies(vec![entry_hash.into()])))?))),
                    HostContext::ValidationPackage(_) => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&Ok::<ValidationPackageCallbackResult, ()>(ValidationPackageCallbackResult::UnresolvedDependencies(vec![entry_hash.into()])))?))),
                },
            }
    })
}

#[cfg(test)]
pub mod test {
    use holochain_util::tokio_helper;
    use ::fixt::prelude::fixt;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use crate::core::ribosome::HostFnWorkspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::core::SourceChainResult;
    use crate::core::ribosome::RibosomeError;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_entry_test<'a>() {
        observability::test_run().ok();
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone()).await.unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace.clone();

        // get the result of a commit entry
        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::MustGet, "create_entry", ()).unwrap();

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            SourceChainResult::Ok(workspace.source_chain().chain_head()?.0)
        })
        .unwrap();

        assert_eq!(&chain_head, &output);

        let round_element: Element =
            crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_valid_element", output).unwrap();

        let round_entry = round_element.entry().to_app_option::<Something>().unwrap().unwrap();

        assert_eq!(
            &round_entry,
            &Something(vec![1, 2, 3])
        );

        let fail_header_hash = HeaderHash::from_raw_32([0; 32].to_vec());

        let element_fail: Result<Element, RibosomeError> = crate::call_test_ribosome!(
            host_access,
            TestWasm::MustGet,
            "must_get_valid_element",
            fail_header_hash
        );

        match element_fail {
            Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
                "Failed to get Element uhCkkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
                e,
            ),
            _ => unreachable!(),
        };

        let signed_header: SignedHeaderHashed = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_header", output).unwrap();

        assert_eq!(
            &signed_header,
            round_element.signed_header(),
        );

        let header_fail: Result<SignedHeaderHashed, RibosomeError> = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_header", fail_header_hash);

        match header_fail {
            Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
                "Failed to get SignedHeaderHashed uhCkkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
                e,
            ),
            _ => unreachable!(),
        }

        let entry_hash = match signed_header.header() {
            Header::Create(create) => create.entry_hash.clone(),
            _ => unreachable!(),
        };

        let entry: EntryHashed = crate::call_test_ribosome!(
            host_access,
            TestWasm::MustGet,
            "must_get_entry",
            entry_hash
        ).unwrap();

        assert_eq!(
            &ElementEntry::Present(entry.as_content().clone()),
            round_element.entry(),
        );

        let fail_entry_hash = EntryHash::from_raw_32(vec![0; 32]);

        let entry_fail: Result<EntryHashed, RibosomeError> = crate::call_test_ribosome!(host_access, TestWasm::MustGet, "must_get_entry", fail_entry_hash);

        match entry_fail {
            Err(RibosomeError::WasmError(WasmError::Host(e))) => assert_eq!(
                "Failed to get EntryHashed uhCEkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACZ9h_C",
                e,
            ),
            _ => unreachable!(),
        }
    }
}