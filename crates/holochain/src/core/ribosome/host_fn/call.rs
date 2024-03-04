use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCall;
use futures::future::join_all;
use holochain_nonce::fresh_nonce;
use holochain_p2p::HolochainP2pDnaT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn call(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<Call>,
) -> Result<Vec<ZomeCallResponse>, RuntimeError> {
    let results: Vec<Result<ZomeCallResponse, RuntimeError>> =
        tokio_helper::block_forever_on(async move {
            join_all(inputs.into_iter().map(|input| async {
                // The line below was added when migrating to rust edition 2021, per
                // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
                let _ = &input;
                let Call {
                    target,
                    zome_name,
                    fn_name,
                    cap_secret,
                    payload,
                } = input;

                match (&target, HostFnAccess::from(&call_context.host_context())) {
                    (
                        CallTarget::ConductorCell(_),
                        HostFnAccess {
                            write_workspace: Permission::Allow,
                            agent_info: Permission::Allow,
                            ..
                        },
                    )
                    | (
                        CallTarget::NetworkAgent(_),
                        HostFnAccess {
                            write_network: Permission::Allow,
                            agent_info: Permission::Allow,
                            ..
                        },
                    ) => {
                        let provenance = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain to know provenance")
                            .agent_pubkey()
                            .clone();
                        let (nonce, expires_at) =
                            fresh_nonce(Timestamp::now()).map_err(|e| -> RuntimeError {
                                wasm_error!(WasmErrorInner::Host(e.to_string())).into()
                            })?;

                        let result: Result<ZomeCallResponse, RuntimeError> = match target {
                            CallTarget::NetworkAgent(target_agent) => {
                                let zome_call_unsigned = ZomeCallUnsigned {
                                    provenance: provenance.clone(),
                                    cell_id: CellId::new(
                                        ribosome.dna_def().as_hash().clone(),
                                        target_agent.clone(),
                                    ),
                                    zome_name,
                                    fn_name,
                                    cap_secret,
                                    payload,
                                    nonce,
                                    expires_at,
                                };
                                match call_context
                                    .host_context()
                                    .network()
                                    .call_remote(
                                        provenance.clone(),
                                        zome_call_unsigned
                                            .provenance
                                            .sign_raw(
                                                call_context.host_context.keystore(),
                                                zome_call_unsigned.data_to_sign().map_err(
                                                    |e| -> RuntimeError {
                                                        wasm_error!(e.to_string()).into()
                                                    },
                                                )?,
                                            )
                                            .await
                                            .map_err(|e| -> RuntimeError {
                                                wasm_error!(WasmErrorInner::Host(e.to_string()))
                                                    .into()
                                            })?,
                                        target_agent,
                                        zome_call_unsigned.zome_name,
                                        zome_call_unsigned.fn_name,
                                        zome_call_unsigned.cap_secret,
                                        zome_call_unsigned.payload,
                                        zome_call_unsigned.nonce,
                                        zome_call_unsigned.expires_at,
                                    )
                                    .await
                                {
                                    Ok(serialized_bytes) => {
                                        ZomeCallResponse::try_from(serialized_bytes)
                                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })
                                    }
                                    Err(e) => Ok(ZomeCallResponse::NetworkError(e.to_string())),
                                }
                            }
                            CallTarget::ConductorCell(target_cell) => {
                                let cell_id_result: Result<CellId, RuntimeError> = match target_cell
                                {
                                    CallTargetCell::OtherRole(role_name) => {
                                        let this_cell_id = call_context
                                            .host_context()
                                            .call_zome_handle()
                                            .cell_id()
                                            .clone();
                                        call_context
                                            .host_context()
                                            .call_zome_handle()
                                            .find_cell_with_role_alongside_cell(
                                                &this_cell_id,
                                                &role_name,
                                            )
                                            .await
                                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })
                                            .and_then(|c| {
                                                c.ok_or_else(|| {
                                                    wasmer::RuntimeError::from(wasm_error!(
                                                        WasmErrorInner::Host(format!(
                                                            "Role not found: {role_name}"
                                                        ))
                                                    ))
                                                })
                                            })
                                    }
                                    CallTargetCell::OtherCell(cell_id) => Ok(cell_id),
                                    CallTargetCell::Local => Ok(call_context
                                        .host_context()
                                        .call_zome_handle()
                                        .cell_id()
                                        .clone()),
                                };
                                match cell_id_result {
                                    Ok(cell_id) => {
                                        let zome_call_unsigned = ZomeCallUnsigned {
                                            cell_id,
                                            zome_name,
                                            fn_name,
                                            payload,
                                            cap_secret,
                                            provenance,
                                            nonce,
                                            expires_at,
                                        };
                                        let call = ZomeCall::try_from_unsigned_zome_call(
                                            call_context.host_context.keystore(),
                                            zome_call_unsigned,
                                        )
                                        .await
                                        .map_err(|e| -> RuntimeError {
                                            wasm_error!(WasmErrorInner::Host(e.to_string())).into()
                                        })?;
                                        match call_context
                                            .host_context()
                                            .call_zome_handle()
                                            .call_zome(
                                                call,
                                                call_context
                                                    .host_context()
                                                    .workspace_write()
                                                    .clone()
                                                    .try_into()
                                                    .expect(
                                                        "Must have source chain to make zome call",
                                                    ),
                                            )
                                            .await
                                        {
                                            Ok(Ok(zome_call_response)) => Ok(zome_call_response),
                                            Ok(Err(ribosome_error)) => Err(wasm_error!(
                                                WasmErrorInner::Host(ribosome_error.to_string())
                                            )
                                            .into()),
                                            Err(conductor_api_error) => {
                                                Err(wasm_error!(WasmErrorInner::Host(
                                                    conductor_api_error.to_string()
                                                ))
                                                .into())
                                            }
                                        }
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                        };
                        result
                    }
                    _ => Err(wasm_error!(WasmErrorInner::Host(
                        RibosomeError::HostFnPermissions(
                            call_context.zome.zome_name().clone(),
                            call_context.function_name().clone(),
                            "call".into(),
                        )
                        .to_string(),
                    ))
                    .into()),
                }
            }))
            .await
        });
    let results: Result<Vec<_>, _> = results.into_iter().collect();
    results
}

#[cfg(test)]
pub mod wasm_test {
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use hdk::prelude::AgentInfo;
    use holo_hash::ActionHash;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use rusqlite::named_params;

    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::sweettest::SweetAgents;
    use crate::test_utils::new_zome_call_unsigned;
    use holochain_conductor_api::ZomeCall;
    use holochain_sqlite::prelude::DatabaseResult;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_test() {
        holochain_trace::test_run().ok();
        let test_wasm = TestWasm::WhoAmI;
        let (dna_file_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![test_wasm]).await;

        let dna_file_2 = dna_file_1
            .clone()
            .with_network_seed("CLONE".to_string())
            .await;

        let mut conductor = SweetConductor::from_standard_config().await;
        let (alice_pubkey, _) = SweetAgents::alice_and_bob();

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                [&alice_pubkey],
                [
                    &("role1".to_string(), dna_file_1),
                    &("role2".to_string(), dna_file_2),
                ],
            )
            .await
            .unwrap();

        let ((cell1, cell2),) = apps.into_tuples();

        let zome1 = cell1.zome(test_wasm);
        let zome2 = cell2.zome(test_wasm);

        let _: () = conductor.call(&zome2, "set_access", ()).await;

        {
            let agent_info: AgentInfo = conductor
                .call(&zome1, "who_are_they_local", cell2.cell_id())
                .await;
            assert_eq!(agent_info.agent_initial_pubkey, alice_pubkey);
            assert_eq!(agent_info.agent_latest_pubkey, alice_pubkey);
        }
        {
            let agent_info: AgentInfo = conductor.call(&zome1, "who_are_they_role", "role2").await;
            assert_eq!(agent_info.agent_initial_pubkey, alice_pubkey);
            assert_eq!(agent_info.agent_latest_pubkey, alice_pubkey);
        }
    }

    /// When calling the same cell we need to make sure
    /// the "as at" doesn't cause the original zome call to fail
    /// when they are both writing (moving the source chain forward)
    #[tokio::test(flavor = "multi_thread")]
    async fn call_the_same_cell() {
        holochain_trace::test_run().ok();

        let zomes = vec![TestWasm::WhoAmI, TestWasm::Create];
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(zomes).await;
        let mut conductor = SweetConductor::from_standard_config().await;
        let (alice,) = conductor
            .setup_app("app", &[dna])
            .await
            .unwrap()
            .into_tuple();

        let handle = conductor.raw_handle();

        let zome_call_unsigned =
            new_zome_call_unsigned(&alice.cell_id(), "call_create_entry", (), TestWasm::Create)
                .unwrap();
        let zome_call =
            ZomeCall::try_from_unsigned_zome_call(handle.keystore(), zome_call_unsigned)
                .await
                .unwrap();
        let result = handle.call_zome(zome_call).await;
        assert_matches!(result, Ok(Ok(ZomeCallResponse::Ok(_))));

        // Get the action hash of that entry
        let action_hash: ActionHash =
            unwrap_to::unwrap_to!(result.unwrap().unwrap() => ZomeCallResponse::Ok)
                .decode()
                .unwrap();

        // Check alice's source chain contains the new value
        let has_hash: bool = handle
            .get_spaces()
            .authored_db(alice.dna_hash())
            .unwrap()
            .read_async(move |txn| -> DatabaseResult<bool> {
                Ok(txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE action_hash = :hash)",
                    named_params! {
                        ":hash": action_hash
                    },
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(has_hash);
    }

    /// test calling a different zome
    /// in a different cell.
    // FIXME: we should NOT be able to do a "bridge" call to another cell in a different app, by a different agent!
    //        Local bridge calls are always within the same app. So this test is testing something that should
    //        not be supported.
    #[tokio::test(flavor = "multi_thread")]
    async fn bridge_call() {
        holochain_trace::test_run().ok();

        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

        let mut conductor = SweetConductor::from_standard_config().await;
        let (alice, bob) = SweetAgents::two(conductor.keystore()).await;

        let apps = conductor
            .setup_app_for_agents("app", &[alice.clone(), bob.clone()], &[dna_file])
            .await
            .unwrap();
        let ((alice,), (_bobbo,)) = apps.into_tuples();

        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::WhoAmI]).await;
        let apps = conductor
            .setup_app_for_agents("app2", &[bob.clone()], &[dna_file])
            .await
            .unwrap();
        let ((bobbo2,),) = apps.into_tuples();
        let action_hash: ActionHash = conductor
            .call(
                &bobbo2.zome(TestWasm::WhoAmI),
                "call_create_entry",
                alice.cell_id().clone(),
            )
            .await;

        // Check alice's source chain contains the new value
        let has_hash: bool = alice
            .dht_db()
            .read_async(move |txn| -> DatabaseResult<bool> {
                Ok(txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE action_hash = :hash)",
                    named_params! {
                        ":hash": action_hash
                    },
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(has_hash);
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can call a fn on a remote
    async fn call_remote_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::WhoAmI).await;

        let _: () = conductor.call(&bob, "set_access", ()).await;
        let agent_info: AgentInfo = conductor
            .call(&alice, "whoarethey", bob_pubkey.clone())
            .await;
        assert_eq!(agent_info.agent_initial_pubkey, bob_pubkey);
        assert_eq!(agent_info.agent_latest_pubkey, bob_pubkey);
    }
}
