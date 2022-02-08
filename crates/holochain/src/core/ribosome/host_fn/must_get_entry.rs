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
    use holochain_state::prelude::*;
    use holochain_types::prelude::*;
    use unwrap_to::unwrap_to;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    test_entry_impl!(Something);

    const ENTRY_DEF_ID: &str = "something";

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_entry_test<'a>() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            alice_host_fn_caller,
            ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        let entry = Entry::try_from(Something(vec![1, 2, 3])).unwrap();
        let header_hash = alice_host_fn_caller.commit_entry(entry.clone(), ENTRY_DEF_ID).await;

        let dht_env = conductor
            .inner_handle()
            .get_dht_env(alice.cell_id().dna_hash())
            .unwrap();

        // When we first get the element it will return because we haven't yet
        // set the validation status.
        let element: Element = conductor
            .call(&bob, "must_get_valid_element", header_hash.clone())
            .await;

        let signature = element.signature().clone();
        let header = element.header().clone();
        let maybe_entry_box: Option<Box<Entry>> = element
            .entry()
            .as_option()
            .cloned()
            .map(|entry| Box::new(entry));
        let entry_state = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::try_from(header.clone()).unwrap(),
            maybe_entry_box.clone().unwrap(),
        ));
        let element_state = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature,
            header.clone(),
            maybe_entry_box,
        ));
        dht_env
            .conn()
            .unwrap()
            .with_commit_sync(|txn| {
                set_validation_status(
                    txn,
                    element_state.as_hash().clone(),
                    ValidationStatus::Rejected,
                )
                .unwrap();
                set_validation_status(
                    txn,
                    entry_state.as_hash().clone(),
                    ValidationStatus::Rejected,
                )
            })
            .unwrap();

        // Must get entry returns the entry if it exists regardless of the
        // validation status.
        let must_get_entry: EntryHashed = conductor
            .call(&bob, "must_get_entry", header.entry_hash().clone())
            .await;
        assert_eq!(
            Entry::from(must_get_entry),
            entry,
        );

        // Must get header returns the header if it exists regardless of the
        // validation status.
        let must_get_header: SignedHeaderHashed = conductor
            .call(&bob, "must_get_header", header_hash.clone())
            .await;
        assert_eq!(
            must_get_header.header(),
            &header,
        );

        // Must get VALID element ONLY returns the element if it is valid.
        let must_get_valid_element: Result<Element, _> = conductor
            .call_fallible(&bob, "must_get_valid_element", header_hash)
            .await;
        assert!(must_get_valid_element.is_err());

        let bad_entry_hash = EntryHash::from_raw_32(vec![1; 32]);
        let bad_must_get_entry: Result<EntryHashed, _> = conductor.call_fallible(&bob, "must_get_entry", bad_entry_hash).await;
        assert!(bad_must_get_entry.is_err());

        let bad_header_hash = HeaderHash::from_raw_32(vec![2; 32]);
        let bad_must_get_header: Result<SignedHeaderHashed, _> = conductor.call_fallible(&bob, "must_get_header", bad_header_hash).await;
        assert!(bad_must_get_header.is_err());
    }
}
