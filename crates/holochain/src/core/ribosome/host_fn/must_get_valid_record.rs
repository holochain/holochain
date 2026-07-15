use crate::core::ribosome::host_fn::cascade_from_call_context;
use crate::core::ribosome::{CallContext, Ribosome};
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use holochain_cascade::{CascadeImpl, CascadeOptions};
use holochain_p2p::actor::NetworkRequestOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(_ribosome, call_context))
)]
pub fn must_get_valid_record(
    _ribosome: Arc<Ribosome>,
    call_context: Arc<CallContext>,
    input: MustGetValidRecordInput,
) -> Result<Record, RuntimeError> {
    tracing::debug!("begin must_get_valid_record");
    let ret = match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let action_hash = input.into_inner();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                use crate::core::ribosome::ValidateHostAccess;
                let (cascade, opt) = match call_context.host_context {
                    HostContext::Validate(ValidateHostAccess { is_inline, .. }) => {
                        if is_inline {
                            (
                                cascade_from_call_context(&call_context),
                                CascadeOptions {
                                    network_request_options:
                                        NetworkRequestOptions::must_get_options(),
                                    get_options: GetOptions::network(),
                                },
                            )
                        } else {
                            (
                                CascadeImpl::from_workspace_stores(workspace.stores(), None)
                                    .with_zome_call_origin(
                                        call_context.zome.zome_name(),
                                        call_context.function_name(),
                                    ),
                                CascadeOptions {
                                    network_request_options:
                                        NetworkRequestOptions::must_get_options(),
                                    get_options: GetOptions::local(),
                                },
                            )
                        }
                    }
                    _ => (
                        cascade_from_call_context(&call_context),
                        CascadeOptions {
                            network_request_options: NetworkRequestOptions::must_get_options(),
                            get_options: GetOptions::network(),
                        },
                    ),
                };
                match cascade
                    .get_record_details(action_hash.clone(), opt)
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    // Only short-circuit as Invalid when running inside the Validate host context.
                    Some(RecordDetails {
                        validation_status: ValidationStatus::Rejected,
                        ..
                    }) if matches!((call_context).host_context, HostContext::Validate(_)) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(ValidateCallbackResult::Invalid(
                                    "Found a record, but it is invalid".to_string()
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into())
                    }
                    Some(RecordDetails {
                        record,
                        validation_status: ValidationStatus::Valid,
                        ..
                    }) => Ok(record),
                    _ => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheckV1(_)
                        | HostContext::GenesisSelfCheckV2(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                            format!("Failed to get Record {action_hash}")
                        ))
                        .into()),
                        HostContext::Init(_) => Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::Hashes(vec![action_hash.into()],)
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
                                                vec![action_hash.into()],
                                            )
                                        )
                                    )
                                    .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
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
                "must_get_valid_record".into(),
            )
            .to_string(),
        ))
        .into()),
    };
    tracing::debug!(?ret);
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::ribosome::{guest_callback::validate::ValidateHostAccess, InvocationAuth},
        test_utils::RibosomeTestFixture,
    };
    use ::fixt::prelude::*;
    use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::fixt::{ActionFixturator, CreateAction};

    // This test ensures the ValidationStatus::Rejected arm is hit and returns a
    // HostShortCircuit carrying ValidateCallbackResult::Invalid with the expected message.
    #[tokio::test(flavor = "multi_thread")]
    async fn must_get_valid_record_short_circuit_when_invalid_record_found() {
        holochain_trace::test_run();

        let RibosomeTestFixture {
            alice_host_fn_caller,
            alice_cell,
            ..
        } = RibosomeTestFixture::new(TestWasm::Validate).await;

        // Build a StoreRecord op for a Create action.
        let mut create_action = fixt!(Action, CreateAction);
        // Set author to the cell's agent to keep data coherent.
        create_action.header.author = alice_cell.agent_pubkey().clone();
        let create_entry = fixt!(Entry);
        let create_entry_hash = create_action.entry_hash().unwrap().clone();

        // Cache the StoreRecord record into the new DhtStore (integrated, as a
        // fetched op would be) and mark it Rejected, so the cascade's
        // DhtStore-backed get_record_details sees it as invalid.
        let rendered = holochain_types::wire_ops::RenderedOp::new(
            create_action.clone(),
            fixt!(Signature),
            None,
            holochain_zome_types::op::ChainOpType::CreateRecord,
        )
        .unwrap();
        let create_op_hash = rendered.op_hash.clone();
        let rendered_ops = holochain_types::wire_ops::RenderedOps {
            entry: Some(holochain_types::prelude::EntryHashed::with_pre_hashed(
                create_entry,
                create_entry_hash,
            )),
            ops: vec![rendered],
            warrant: None,
        };
        alice_host_fn_caller
            .dht_store
            .cache_chain_ops(&rendered_ops)
            .await
            .unwrap();
        alice_host_fn_caller
            .dht_store
            .reject_chain_ops(vec![create_op_hash])
            .await
            .unwrap();

        // Call must_get_valid_record directly through the host function using the `Validate` host context.
        let cell_id = alice_host_fn_caller.zome_path.cell_id().clone();
        let zome_name = alice_host_fn_caller.zome_path.zome_name().clone();
        let workspace = HostFnWorkspaceRead::new(
            alice_host_fn_caller.dht_store.clone(),
            alice_host_fn_caller.keystore.clone(),
            Some(cell_id.agent_pubkey().clone()),
        )
        .await
        .unwrap();
        let call_context = Arc::new(CallContext::new(
            alice_host_fn_caller
                .ribosome
                .dna_def()
                .get_zome(&zome_name)
                .unwrap(),
            "".into(),
            HostContext::Validate(ValidateHostAccess::new(
                workspace,
                Arc::new(alice_host_fn_caller.network.clone()),
                false,
            )),
            InvocationAuth::Cap(cell_id.agent_pubkey().clone(), None),
        ));
        let err = must_get_valid_record(
            Arc::new(alice_host_fn_caller.ribosome.clone()),
            call_context,
            MustGetValidRecordInput::new(create_action.to_hash()),
        )
        .unwrap_err();

        // Extract the HostShortCircuit payload and assert encoded Invalid message is exact.
        let wasm_error: WasmError = err.downcast().unwrap();
        if let WasmError {
            error: WasmErrorInner::HostShortCircuit(bytes),
            ..
        } = wasm_error
        {
            let extern_io: ExternIO =
                decode(&bytes).expect("decode HostShortCircuit into ExternIO");
            let vcr: ValidateCallbackResult = extern_io
                .decode()
                .expect("ExternIO -> ValidateCallbackResult");
            match vcr {
                ValidateCallbackResult::Invalid(msg) => {
                    assert_eq!(msg, "Found a record, but it is invalid".to_string());
                }
                other => panic!("Expected ValidateCallbackResult::Invalid, got {other:?}"),
            }
        } else {
            panic!("Expected WasmErrorInner::HostShortCircuit");
        }
    }
}
