use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::HolochainP2pCellT;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

pub fn call_remote(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CallRemote,
) -> Result<ZomeCallResponse, WasmError> {
    // it is the network's responsibility to handle timeouts and return an Err result in that case
    let result: Result<SerializedBytes, _> = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let mut network = call_context.host_access().network().clone();
        network
            .call_remote(
                input.target_agent_as_ref().to_owned(),
                input.zome_name_as_ref().to_owned(),
                input.fn_name_as_ref().to_owned(),
                input.cap_as_ref().to_owned(),
                input.payload_as_ref().to_owned(),
            )
            .await
    });
    let result = match result {
        Ok(r) => ZomeCallResponse::try_from(r)?,
        Err(e) => ZomeCallResponse::NetworkError(e.to_string()),
    };

    Ok(result)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::conductor::interface::websocket::test_utils::setup_app;
    use crate::conductor::{api::ZomeCall, dna_store::MockDnaStore};
    use crate::core::ribosome::ZomeCallResponse;
    use hdk::prelude::*;
    use holochain_types::app::InstalledCell;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;
    pub use holochain_zome_types::capability::CapSecret;
    use holochain_zome_types::cell::CellId;
    use holochain_zome_types::ExternIO;

    #[tokio::test(threaded_scheduler)]
    /// we can call a fn on a remote
    async fn call_remote_test() {
        // ////////////
        // START DNA
        // ////////////

        let dna_def = DnaDef {
            name: "call_remote_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
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
                cell_id: alice_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "whoarethey".into(),
                payload: ExternIO::encode(
                    &bob_agent_id
                ).unwrap(),
                provenance: alice_agent_id,
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
                        agent_latest_pubkey: bob_agent_id,
                    },
                );
            }
            _ => unreachable!(),
        }

        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap();
    }
}
