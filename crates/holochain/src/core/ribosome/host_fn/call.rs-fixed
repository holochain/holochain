use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCall;
use crate::core::ribosome::CallContext;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

pub fn call(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    call: Call,
) -> Result<ZomeCallResponse, WasmError> {

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
    Ok(tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        conductor_handle
            .call_zome(invocation, workspace)
            .await
            .map_err(Box::new)
    })
    .map_err(|conductor_api_error| WasmError::Host(conductor_api_error.to_string()))?
    .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?)
}

#[cfg(test)]
pub mod wasm_test {
    use std::convert::TryFrom;

    use hdk3::prelude::AgentInfo;
    use hdk3::prelude::CellId;
    use hdk3::prelude::WasmError;
    use holo_hash::HeaderHash;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_types::app::InstalledCell;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;
    use holochain_zome_types::ExternIO;
    use holochain_zome_types::ZomeCallResponse;
    use matches::assert_matches;

    use crate::core::ribosome::error::RibosomeError;
    use crate::conductor::{api::ZomeCall, ConductorHandle};
    use crate::test_utils::conductor_setup::ConductorTestData;
    use crate::test_utils::install_app;
    use crate::test_utils::new_zome_call;
    use holochain_state::element_buf::ElementBuf;

    #[tokio::test(threaded_scheduler)]
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

        // ALICE CALLING Zome API requiring NO cap grant
        let cap_init = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "whoarethey_local_open".into(),
                payload: ExternIO::encode(
                    &bob_cell_id
                ).unwrap(),
                provenance: alice_agent_id.clone(),
            })
            .await;
        assert_matches!(cap_init, Ok(Ok(ZomeCallResponse::Ok(_))));

        // ALICE CALLING Zome API requiring cap grant from "set_access"

        let cap_fail = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "whoarethey_local".into(),
                payload: ExternIO::encode(
                    &bob_cell_id
                ).unwrap(),
                provenance: alice_agent_id.clone(),
            })
            .await;

	// eg. WasmError(Zome("inner function \'whoarethey_local\' failed: UnauthorizedZomeCall(CellId(DnaHash(uhC0kCnAyTh2eEgAex-paR8zCKrmcz25tA8qkrk2UdfOJGkEjuPRb), AgentPubKey(uhCAkmrkoAHPVf_eufG7eC5fm6QKrW5pPMoktvG5LOC0SnJ4vV1Uv)), ZomeName(\"whoami\"), FunctionName(\"whoami\"), AgentPubKey(uhCAke1j8Z2a-_min0h0pGuEMcYlo_V1l1mt9OtBuywKmHlg4L_R-))")),

	match cap_fail {
            Ok(Err(RibosomeError::WasmError(WasmError::Guest(s)))) => {
		assert!(s.contains(
		    "inner function \'whoarethey_local\' failed: UnauthorizedZomeCall"));
	    },
	    other => println!("Unknown response: {:?}", other),
	};


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
                fn_name: "whoarethey_local".into(),
                payload: ExternIO::encode(
                    &bob_cell_id
                ).unwrap(),
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

    /// When calling the same cell we need to make sure the "as at" doesn't cause the original zome
    /// call to fail when they are both writing (moving the source chain forward).  Also tests the
    /// case where both Zome init() functions write things (CapGrant, Entries) to the source-chain.
    #[tokio::test(threaded_scheduler)]
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
        let alice_source_chain =
            ElementBuf::authored(alice_call_data.env.clone().into(), true).unwrap();
        let el = alice_source_chain.get_element(&header_hash).unwrap();
        assert_matches!(el, Some(_));

        conductor_test.shutdown_conductor().await;
    }

    /// test calling a different zome
    /// in a different cell.
    #[tokio::test(threaded_scheduler)]
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
        let alice_source_chain =
            ElementBuf::authored(alice_call_data.env.clone().into(), true).unwrap();
        let el = alice_source_chain.get_element(&header_hash).unwrap();
        assert_matches!(el, Some(_));

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
                uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
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
