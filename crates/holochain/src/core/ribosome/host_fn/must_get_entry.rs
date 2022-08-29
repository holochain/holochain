use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetEntryInput,
) -> Result<EntryHashed, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let entry_hash = input.into_inner();
            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = match call_context.host_context {
                    HostContext::Validate(_) => Cascade::from_workspace(workspace.stores(), None),
                    _ => Cascade::from_workspace_network(
                        &workspace,
                        call_context.host_context.network().clone(),
                    ),
                };
                let result: Result<_, RuntimeError> = match cascade
                    .retrieve_entry(entry_hash.clone(), NetworkGetOptions::must_get_options())
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    Some(entry) => Ok(entry),
                    None => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheck(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                            format!("Failed to get EntryHashed {}", entry_hash)
                        ))
                        .into()),
                        HostContext::Init(_) => Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::Hashes(vec![entry_hash.into()],)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                        ))
                        .into()),
                        HostContext::Validate(_) => {
                            Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        &ValidateCallbackResult::UnresolvedDependencies(
                                            UnresolvedDependencies::Hashes(
                                                vec![entry_hash.into(),]
                                            )
                                        ),
                                    )
                                    .map_err(
                                        |e| -> RuntimeError { wasm_error!(e.into()).into() }
                                    )?
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            ))
                            .into())
                        }
                        HostContext::ValidationPackage(_) => {
                            Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidationPackageCallbackResult::UnresolvedDependencies(
                                            vec![entry_hash.into(),]
                                        ),
                                    )
                                    .map_err(|e| wasm_error!(e.into()))?
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            ))
                            .into())
                        }
                    },
                };
                result
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_entry".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
pub mod test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::test_entry_impl;
    use hdk::prelude::*;
    use holochain_state::prelude::*;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use unwrap_to::unwrap_to;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    test_entry_impl!(Something);

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
        let action_hash = alice_host_fn_caller
            .commit_entry(
                entry.clone(),
                EntryDefLocation::app(0, EntryDefIndex(0)),
                EntryVisibility::Public,
            )
            .await;

        let dht_db = conductor
            .inner_handle()
            .get_dht_db(alice.cell_id().dna_hash())
            .unwrap();

        // When we first get the record it will return because we haven't yet
        // set the validation status.
        let record: Record = conductor
            .call(&bob, "must_get_valid_record", action_hash.clone())
            .await;

        let signature = record.signature().clone();
        let action = record.action().clone();
        let maybe_entry_box: Option<Box<Entry>> = record
            .entry()
            .as_option()
            .cloned()
            .map(|entry| Box::new(entry));
        let entry_state = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryAction::try_from(action.clone()).unwrap(),
            maybe_entry_box.clone().unwrap(),
        ));
        let record_state = DhtOpHashed::from_content_sync(DhtOp::StoreRecord(
            signature,
            action.clone(),
            maybe_entry_box,
        ));
        dht_db
            .conn()
            .unwrap()
            .with_commit_sync(|txn| {
                set_validation_status(txn, record_state.as_hash(), ValidationStatus::Rejected)
                    .unwrap();
                set_validation_status(txn, entry_state.as_hash(), ValidationStatus::Rejected)
            })
            .unwrap();

        // Must get entry returns the entry if it exists regardless of the
        // validation status.
        let must_get_entry: EntryHashed = conductor
            .call(&bob, "must_get_entry", action.entry_hash().clone())
            .await;
        assert_eq!(Entry::from(must_get_entry), entry,);

        // Must get action returns the action if it exists regardless of the
        // validation status.
        let must_get_action: SignedActionHashed = conductor
            .call(&bob, "must_get_action", action_hash.clone())
            .await;
        assert_eq!(must_get_action.action(), &action,);

        // Must get VALID record ONLY returns the record if it is valid.
        let must_get_valid_record: Result<Record, _> = conductor
            .call_fallible(&bob, "must_get_valid_record", action_hash)
            .await;
        assert!(must_get_valid_record.is_err());

        let bad_entry_hash = EntryHash::from_raw_32(vec![1; 32]);
        let bad_must_get_entry: Result<EntryHashed, _> = conductor
            .call_fallible(&bob, "must_get_entry", bad_entry_hash)
            .await;
        assert!(bad_must_get_entry.is_err());

        let bad_action_hash = ActionHash::from_raw_32(vec![2; 32]);
        let bad_must_get_action: Result<SignedActionHashed, _> = conductor
            .call_fallible(&bob, "must_get_action", bad_action_hash)
            .await;
        assert!(bad_must_get_action.is_err());
    }
}
