use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCall;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

pub fn call(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    call: Call,
) -> Result<ZomeCallResponse, WasmError> {
    match HostFnAccess::from(&call_context.host_access()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            // Get the conductor handle
            let host_access = call_context.host_access();
            let conductor_handle = host_access.call_zome_handle();
            let workspace = host_access.workspace();

            // Get the cell id if it's not passed in
            let cell_id = call
                .to_cell
                .unwrap_or_else(|| conductor_handle.cell_id().clone());

            let zome_name = call.zome_name.clone();

            // Create the invocation for this call
            let invocation = ZomeCall {
                cell_id,
                zome_name,
                cap: call.cap,
                fn_name: call.fn_name,
                payload: call.payload,
                provenance: call.provenance,
            };

            // Make the call using this workspace
            tokio_helper::block_forever_on(async move {
                conductor_handle
                    .call_zome(invocation, workspace)
                    .await
                    .map_err(Box::new)
            })
            .map_err(|conductor_api_error| WasmError::Host(conductor_api_error.to_string()))?
            .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))
        },
        _ => unreachable!(),
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
                cap: None,
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
                cap: None,
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
                    agent_info,
                    AgentInfo {
                        agent_initial_pubkey: bob_agent_id.clone(),
                        agent_latest_pubkey: bob_agent_id.clone(),
                    },
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
        let has_hash: bool = fresh_reader_test(alice_call_data.env.clone(), |txn| {
            txn.query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE header_hash = :hash AND is_authored = 1)",
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
        let has_hash: bool = fresh_reader_test(alice_call_data.env.clone(), |txn| {
            txn.query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE header_hash = :hash AND is_authored = 1)",
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
}
