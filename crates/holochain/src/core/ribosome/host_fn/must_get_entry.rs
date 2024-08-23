use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::{Cascade, CascadeImpl};
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
#[cfg_attr(feature = "instrument", tracing::instrument(skip(_ribosome, call_context)))]
pub fn must_get_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetEntryInput,
) -> Result<EntryHashed, RuntimeError> {
    tracing::debug!("begin must_get_entry");
    let ret = match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let entry_hash = input.into_inner();
            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                use crate::core::ribosome::ValidateHostAccess;
                let cascade = match call_context.host_context {
                    HostContext::Validate(ValidateHostAccess { is_inline, .. }) => {
                        if is_inline {
                            CascadeImpl::from_workspace_and_network(
                                &workspace,
                                call_context.host_context.network().clone(),
                            )
                        } else {
                            CascadeImpl::from_workspace_stores(workspace.stores(), None)
                        }
                    }
                    _ => CascadeImpl::from_workspace_and_network(
                        &workspace,
                        call_context.host_context.network().clone(),
                    ),
                };
                match cascade
                    .retrieve_entry(entry_hash.clone(), NetworkGetOptions::must_get_options())
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    Some((entry, _)) => Ok(entry),
                    None => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheckV1(_)
                        | HostContext::GenesisSelfCheckV2(_)
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
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
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
                                    .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            ))
                            .into())
                        }
                    },
                }
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
    };
    tracing::debug!(?ret);
    ret
}

#[cfg(test)]
pub mod test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::test_entry_impl;
    use hdk::prelude::*;
    use holochain_state::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use unwrap_to::unwrap_to;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    test_entry_impl!(Something);

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_entry_test() {
        holochain_trace::test_run();
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
            .raw_handle()
            .get_dht_db(alice.cell_id().dna_hash())
            .unwrap();

        // When we first get the record it will return because we haven't yet
        // set the validation status.
        let record: Record = conductor
            .call(&bob, "must_get_valid_record", action_hash.clone())
            .await;

        let signature = record.signature().clone();
        let action = record.action().clone();
        let record_entry: RecordEntry = record.entry().clone();
        let entry = record_entry.clone().into_option().unwrap();
        let entry_state = DhtOpHashed::from_content_sync(ChainOp::StoreEntry(
            signature.clone(),
            NewEntryAction::try_from(action.clone()).unwrap(),
            entry.clone(),
        ));
        let record_state = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
            signature,
            action.clone(),
            record_entry,
        ));
        dht_db
            .write_async(move |txn| -> StateMutationResult<()> {
                set_validation_status(txn, record_state.as_hash(), ValidationStatus::Rejected)?;
                set_validation_status(txn, entry_state.as_hash(), ValidationStatus::Rejected)?;

                Ok(())
            })
            .await
            .unwrap();

        // Must get entry returns the entry if it exists regardless of the
        // validation status.
        let must_get_entry: EntryHashed = conductor
            .call(&bob, "must_get_entry", action.entry_hash())
            .await;
        assert_eq!(Entry::from(must_get_entry), entry);

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
