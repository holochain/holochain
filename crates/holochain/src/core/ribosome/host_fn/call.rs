use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCall;
use futures::future::join_all;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use holochain_p2p::HolochainP2pDnaT;

pub fn call(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<Call>,
) -> Result<Vec<ZomeCallResponse>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Result<ZomeCallResponse, _>, _>> =
                tokio_helper::block_forever_on(async move {
                    join_all(inputs.into_iter().map(|input| async {
                        let Call {
                            target,
                            zome_name,
                            fn_name,
                            cap_secret,
                            payload,
                        } = input;
                        let provenance = call_context.host_context.workspace().source_chain()
                        .as_ref().expect("Must have source chain to know provenance")
                        .agent_pubkey()
                        .clone();

                        match target {
                            CallTarget::Agent(target_agent) => {
                                call_context
                                .host_context()
                                .network()
                                .call_remote(provenance, target_agent, zome_name, fn_name, cap_secret, payload)
                                .await
                            },
                            CallTarget::Cell(target_cell) => {
                                let cell_id = match target_cell {
                                    CallTargetCell::Other(cell_id) => cell_id,
                                    CallTargetCell::Local => call_context
                                        .host_context()
                                        .call_zome_handle()
                                        .cell_id()
                                        .clone(),
                                };
                                let invocation = ZomeCall {
                                    cell_id,
                                    zome_name,
                                    fn_name,
                                    payload,
                                    cap_secret,
                                    provenance,
                                };
                                call_context
                                    .host_context()
                                    .call_zome_handle()
                                    .call_zome(
                                        invocation,
                                        call_context
                                            .host_context()
                                            .workspace_write()
                                            .clone()
                                            .try_into()
                                            .expect("Must have source chain to make zome call"),
                                    )
                                    .await
                            }
                        }
                    }))
                    .await
                });
            let results: Result<Vec<_>, _> = results
                .into_iter()
                .map(|result| match result {
                    Ok(v) => match v {
                        Ok(v) => Ok(v),
                        Err(ribosome_error) => Err(WasmError::Host(ribosome_error.to_string())),
                    },
                    Err(conductor_api_error) => {
                        Err(WasmError::Host(conductor_api_error.to_string()))
                    }
                })
                .collect();
            Ok(results?)
        }
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "call".into()
        ).to_string()))
    }
}

#[cfg(test)]
pub mod wasm_test {
    use std::convert::TryFrom;

    use hdk::prelude::AgentInfo;
    use hdk::prelude::CellId;
    use holo_hash::HeaderHash;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::prelude::fresh_reader_test;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;
    use holochain_zome_types::ExternIO;
    use holochain_zome_types::ZomeCallResponse;
    use matches::assert_matches;
    use rusqlite::named_params;
    use crate::test_utils::setup_app;

    use crate::conductor::{api::ZomeCall, ConductorHandle};
    use crate::test_utils::conductor_setup::ConductorTestData;
    use crate::test_utils::install_app;
    use crate::test_utils::new_zome_call;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_test() {
        observability::test_run().ok();

        let zomes = vec![TestWasm::WhoAmI];
        let mut conductor_test = ConductorTestData::two_agents(zomes, true).await;
        let handle = conductor_test.handle();
        let bob_cell_id = conductor_test.bob_call_data().unwrap().cell_id.clone();
        let alice_call_data = conductor_test.alice_call_data();
        let alice_cell_id = &alice_call_data.cell_id;
        let alice_agent_id = alice_cell_id.agent_pubkey();
        let bob_agent_id = bob_cell_id.agent_pubkey();

        // BOB INIT (to do cap grant)

        let _ = handle
            .call_zome(ZomeCall {
                cell_id: bob_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap_secret: None,
                fn_name: "set_access".into(),
                payload: ExternIO::encode(()).unwrap(),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap();

        // ALICE DOING A CALL

        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap_secret: None,
                fn_name: "who_are_they_local".into(),
                payload: ExternIO::encode(&bob_cell_id).unwrap(),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let agent_info: AgentInfo = guest_output.decode().unwrap();
                assert_eq!(
                    &agent_info.agent_initial_pubkey,
                    bob_agent_id
                );
                assert_eq!(
                    &agent_info.agent_latest_pubkey,
                    bob_agent_id
                );
            }
            _ => unreachable!(),
        }
        conductor_test.shutdown_conductor().await;
    }

    /// When calling the same cell we need to make sure
    /// the "as at" doesn't cause the original zome call to fail
    /// when they are both writing (moving the source chain forward)
    #[tokio::test(flavor = "multi_thread")]
    async fn call_the_same_cell() {
        observability::test_run().ok();

        let zomes = vec![TestWasm::WhoAmI, TestWasm::Create];
        let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
        let handle = conductor_test.handle();
        let alice_call_data = conductor_test.alice_call_data();
        let alice_cell_id = &alice_call_data.cell_id;

        let invocation =
            new_zome_call(&alice_cell_id, "call_create_entry", (), TestWasm::Create).unwrap();
        let result = handle.call_zome(invocation).await;
        assert_matches!(result, Ok(Ok(ZomeCallResponse::Ok(_))));

        // Get the header hash of that entry
        let header_hash: HeaderHash =
            unwrap_to::unwrap_to!(result.unwrap().unwrap() => ZomeCallResponse::Ok)
                .decode()
                .unwrap();

        // Check alice's source chain contains the new value
        let has_hash: bool = fresh_reader_test(alice_call_data.authored_env.clone(), |txn| {
            txn.query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE header_hash = :hash)",
                named_params! {
                    ":hash": header_hash
                },
                |row| row.get(0),
            )
            .unwrap()
        });
        assert!(has_hash);

        conductor_test.shutdown_conductor().await;
    }

    /// test calling a different zome
    /// in a different cell.
    #[tokio::test(flavor = "multi_thread")]
    async fn bridge_call() {
        observability::test_run().ok();

        let zomes = vec![TestWasm::Create];
        let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
        let handle = conductor_test.handle();
        let alice_call_data = conductor_test.alice_call_data();
        let alice_cell_id = &alice_call_data.cell_id;

        // Install a different dna for bob
        let zomes = vec![TestWasm::WhoAmI];
        let bob_cell_id = install_new_app("bobs_dna", zomes, &handle).await;

        // Call create_entry in the create_entry zome from the whoami zome
        let invocation = new_zome_call(
            &bob_cell_id,
            "call_create_entry",
            alice_cell_id.clone(),
            TestWasm::WhoAmI,
        )
        .unwrap();
        let result = handle.call_zome(invocation).await;
        assert_matches!(result, Ok(Ok(ZomeCallResponse::Ok(_))));

        // Get the header hash of that entry
        let header_hash: HeaderHash =
            unwrap_to::unwrap_to!(result.unwrap().unwrap() => ZomeCallResponse::Ok)
                .decode()
                .unwrap();

        // Check alice's source chain contains the new value
        let has_hash: bool = fresh_reader_test(alice_call_data.authored_env.clone(), |txn| {
            txn.query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE header_hash = :hash)",
                named_params! {
                    ":hash": header_hash
                },
                |row| row.get(0),
            )
            .unwrap()
        });
        assert!(has_hash);

        conductor_test.shutdown_conductor().await;
    }

    async fn install_new_app(
        dna_name: &str,
        zomes: Vec<TestWasm>,
        handle: &ConductorHandle,
    ) -> CellId {
        let dna_file = DnaFile::new(
            DnaDef {
                name: dna_name.to_string(),
                uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                zomes: zomes.clone().into_iter().map(Into::into).collect(),
            },
            zomes.into_iter().map(Into::into),
        )
        .await
        .unwrap();
        let bob_agent_id = fake_agent_pubkey_2();
        let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
        let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());
        let cell_data = vec![(bob_installed_cell, None)];
        install_app("bob_app", cell_data, vec![dna_file], handle.clone()).await;
        bob_cell_id
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can call a fn on a remote
    async fn call_remote_test() {
        // ////////////
        // START DNA
        // ////////////

        let dna_def = DnaDef {
            name: "call_remote_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::WhoAmI.into()].into(),
        };
        let dna_file = DnaFile::new(dna_def, vec![TestWasm::WhoAmI.into()])
            .await
            .unwrap();

        // //////////
        // END DNA
        // //////////

        // ///////////
        // START ALICE
        // ///////////

        let alice_agent_id = fake_agent_pubkey_1();
        let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
        let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

        // /////////
        // END ALICE
        // /////////

        // /////////
        // START BOB
        // /////////

        let bob_agent_id = fake_agent_pubkey_2();
        let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
        let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

        // ///////
        // END BOB
        // ///////

        // ///////////////
        // START CONDUCTOR
        // ///////////////

        let mut dna_store = MockDnaStore::new();

        dna_store.expect_get().return_const(Some(dna_file.clone()));
        dna_store
            .expect_add_dnas::<Vec<_>>()
            .times(2)
            .return_const(());
        dna_store
            .expect_add_entry_defs::<Vec<_>>()
            .times(2)
            .return_const(());

        let (_tmpdir, _app_api, handle) = setup_app(
            vec![(alice_installed_cell, None), (bob_installed_cell, None)],
            dna_store,
        )
        .await;

        // /////////////
        // END CONDUCTOR
        // /////////////

        // BOB INIT (to do cap grant)

        let _ = handle
            .call_zome(ZomeCall {
                cell_id: bob_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap_secret: None,
                fn_name: "set_access".into(),
                payload: ExternIO::encode(()).unwrap(),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap();

        // ALICE DOING A CALL

        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap_secret: None,
                fn_name: "whoarethey".into(),
                payload: ExternIO::encode(&bob_agent_id).unwrap(),
                provenance: alice_agent_id,
            })
            .await
            .unwrap()
            .unwrap();

        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let agent_info: AgentInfo = guest_output.decode().unwrap();
                assert_eq!(
                    agent_info.agent_initial_pubkey,
                    bob_agent_id
                );
                assert_eq!(
                    agent_info.agent_latest_pubkey,
                    bob_agent_id
                );
            }
            _ => unreachable!(),
        }

        let shutdown = handle.take_shutdown_handle().unwrap();
        handle.shutdown();
        shutdown.await.unwrap().unwrap();
    }
}
