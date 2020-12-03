use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::HolochainP2pCellT;
use holochain_zome_types::CallRemoteInput;
use holochain_zome_types::CallRemoteOutput;
use holochain_zome_types::ZomeCallResponse;
use std::convert::TryInto;
use std::sync::Arc;

pub fn call_remote(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CallRemoteInput,
) -> RibosomeResult<CallRemoteOutput> {
    // it is the network's responsibility to handle timeouts and return an Err result in that case
    let result = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
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
    });
    let result = match result {
        Ok(r) => r.try_into()?,
        Err(e) => ZomeCallResponse::NetworkError(e.to_string()),
    };

    Ok(CallRemoteOutput::new(result))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::conductor::dna_store::MockDnaStore;
    use crate::conductor::interface::websocket::test::setup_app;
    use crate::core::ribosome::ZomeCallInvocation;
    use crate::core::ribosome::ZomeCallResponse;
    use hdk3::prelude::*;
    use holochain_types::app::InstalledCell;
    use holochain_types::cell::CellId;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;
    pub use holochain_zome_types::capability::CapSecret;
    use holochain_zome_types::ExternInput;

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
            .call_zome(ZomeCallInvocation {
                cell_id: bob_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "set_access".into(),
                payload: ExternInput::new(().try_into().unwrap()),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap();

        // ALICE DOING A CALL

        let output = handle
            .call_zome(ZomeCallInvocation {
                cell_id: alice_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "whoarethey".into(),
                payload: ExternInput::new(bob_agent_id.clone().try_into().unwrap()),
                provenance: alice_agent_id,
            })
            .await
            .unwrap()
            .unwrap();

        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let response: SerializedBytes = guest_output.into_inner();
                let agent_info: AgentInfo = response.try_into().unwrap();
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
