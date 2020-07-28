use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::SerializedBytes;
use holochain_zome_types::CallRemoteInput;
use holochain_zome_types::CallRemoteOutput;
use std::sync::Arc;

pub fn call_remote(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CallRemoteInput,
) -> RibosomeResult<CallRemoteOutput> {
    // it is the network's responsibility to handle timeouts and return an Err result in that case
    let result: SerializedBytes = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let mut network = call_context.host_access().network().clone();
        let call_remote = input.into_inner();
        network
            .call_remote(
                call_remote.to_agent(),
                call_remote.zome_name(),
                call_remote.fn_name(),
                call_remote.cap(),
                call_remote.request(),
            )
            .await
    })?;

    Ok(CallRemoteOutput::new(result))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::conductor::dna_store::MockDnaStore;
    use crate::conductor::interface::websocket::test::setup_app;
    use crate::core::ribosome::ZomeCallInvocation;
    use crate::core::ribosome::ZomeCallInvocationResponse;
    use hdk3::prelude::*;
    use holochain_types::app::InstalledCell;
    use holochain_types::cell::CellId;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;
    pub use holochain_zome_types::capability::CapSecret;
    use holochain_zome_types::HostInput;

    #[tokio::test(threaded_scheduler)]
    /// we can call a fn on a remote
    async fn call_remote_test() {
        // ////////////
        // START DNA
        // ////////////

        let dna_file = DnaFile::new(
            DnaDef {
                name: "call_remote_test".to_string(),
                uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                zomes: vec![TestWasm::WhoAmI.into()].into(),
            },
            vec![TestWasm::WhoAmI.into()],
        )
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

        // ALICE DOING A CALL

        let output = handle
            .call_zome(ZomeCallInvocation {
                cell_id: alice_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap: CapSecret::default(),
                fn_name: "whoarethey".to_string(),
                payload: HostInput::new(bob_agent_id.clone().try_into().unwrap()),
                provenance: alice_agent_id,
            })
            .await
            .unwrap()
            .unwrap();

        match output {
            ZomeCallInvocationResponse::ZomeApiFn(guest_output) => {
                let response: SerializedBytes = guest_output.into_inner();
                let agent_info: AgentInfo = response.try_into().unwrap();
                assert_eq!(
                    agent_info,
                    AgentInfo {
                        agent_pubkey: bob_agent_id.clone(),
                        agent_initial_pubkey: bob_agent_id.clone(),
                        agent_latest_pubkey: bob_agent_id,
                    },
                );
            }
        }

        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap();
    }
}
