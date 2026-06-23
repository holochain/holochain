use crate::core::ribosome::host_fn::cascade_from_call_context;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::{Cascade, CascadeImpl};
use holochain_p2p::actor::NetworkRequestOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(_ribosome, call_context))
)]
pub fn must_get_entry(
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
                            cascade_from_call_context(&call_context)
                        } else {
                            CascadeImpl::from_workspace_stores(workspace.stores(), None)
                                .with_zome_call_origin(
                                    call_context.zome.zome_name(),
                                    call_context.function_name(),
                                )
                        }
                    }
                    _ => cascade_from_call_context(&call_context),
                };
                match cascade
                    .retrieve_entry(
                        entry_hash.clone(),
                        NetworkRequestOptions::must_get_options(),
                    )
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    Some((entry, _)) => Ok(entry),
                    None => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheckV1(_)
                        | HostContext::GenesisSelfCheckV2(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                            format!("Failed to get EntryHashed {entry_hash}")
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
                                        ValidateCallbackResult::UnresolvedDependencies(
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
    use crate::test_entry_impl;
    use crate::test_utils::RibosomeTestFixture;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::fixt::{CreateFixturator, SignatureFixturator};
    use unwrap_to::unwrap_to;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    test_entry_impl!(Something);

    #[tokio::test(flavor = "multi_thread")]
    #[cfg_attr(target_os = "windows", ignore = "fails on windows")]
    async fn ribosome_must_get_entry_test() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            bob,
            alice_host_fn_caller,
            ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // Cache a record as if fetched from the network: integrated and
        // initially Accepted. Crucially this is `locally_validated = false`, so
        // it can later be transitioned to Rejected — `reject_chain_ops` only
        // transitions network-cached ops, never an agent's own authored,
        // locally-validated ops. The action's weight is defaulted so it survives
        // the v2 round-trip identically (the v2 model drops weight).
        let entry = Entry::try_from(Something(vec![1, 2, 3])).unwrap();
        let mut create = fixt!(Create);
        create.weight = Default::default();
        let entry_hash = create.entry_hash.clone();
        let action = Action::Create(create);
        let action_hash = action.to_hash();

        let rendered = holochain_types::dht_op::RenderedOp::new(
            action.clone(),
            fixt!(Signature),
            None,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .unwrap();
        let (_, record_op_hash) = holochain_types::dht_op::ChainOpUniqueForm::op_hash(
            holochain_zome_types::op::ChainOpType::StoreRecord,
            action.clone(),
        )
        .unwrap();
        let rendered_ops = holochain_types::dht_op::RenderedOps {
            entry: Some(EntryHashed::with_pre_hashed(entry.clone(), entry_hash)),
            ops: vec![rendered],
            warrant: None,
        };
        alice_host_fn_caller
            .dht_store
            .cache_chain_ops(&rendered_ops)
            .await
            .unwrap();

        // Before rejection the cached record is valid, so must_get_valid_record
        // returns it.
        let _record: Record = conductor
            .call(&bob, "must_get_valid_record", action_hash.clone())
            .await;

        // Reject the cached StoreRecord op.
        alice_host_fn_caller
            .dht_store
            .reject_chain_ops(vec![record_op_hash])
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
