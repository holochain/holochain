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
    let result: ZomeCallResponse = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
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
    })?
    .try_into()?;

    Ok(CallRemoteOutput::new(result))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use std::time::Duration;

    use crate::conductor::{interface::websocket::test::setup_app, ConductorHandle};
    use crate::core::ribosome::ZomeCallInvocation;
    use crate::core::ribosome::ZomeCallResponse;
    use crate::test_utils::conductor_setup::*;
    use crate::test_utils::new_invocation;
    use crate::{conductor::dna_store::MockDnaStore, test_utils::wait_for_integration};
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

    #[tokio::test(threaded_scheduler)]
    async fn call_remote_regression_test() {
        observability::test_run().ok();
        // Check if the correct number of ops are integrated
        // every 100 ms for a maximum of 10 seconds but early exit
        // if they are there.
        let num_attempts = 100;
        let delay_per_attempt = Duration::from_millis(100);

        let zomes = vec![TestWasm::CallRemoteCaller, TestWasm::CallRemoteCallee];
        let conductor_test = ConductorTestData::new(zomes, true).await;
        let ConductorTestData {
            __tmpdir,
            handle,
            alice_call_data,
            bob_call_data,
            ..
        } = conductor_test;
        let bob_call_data = bob_call_data.unwrap();

        // Alice commits base and target
        let invocation = new_invocation(
            &alice_call_data.cell_id,
            "create_and_link_foo",
            (),
            TestWasm::CallRemoteCallee,
        )
        .unwrap();
        handle.call_zome(invocation).await.unwrap().unwrap();

        // Alice gets back the links from the same zome
        let invocation = new_invocation(
            &alice_call_data.cell_id,
            "get_links_on_foo",
            (),
            TestWasm::CallRemoteCallee,
        )
        .unwrap();

        let links: Links = call(&handle, invocation).await;
        assert_eq!(links.into_inner().len(), 1);

        // Integration should have 9 ops in it.
        // Plus another 14 for genesis.
        // Plus 2 cap
        // Plus 2 init
        // Plus 9 ops for the two commits and link
        let expected_count = 9 + 14 + 2 + 2;

        wait_for_integration(
            &bob_call_data.env,
            expected_count,
            num_attempts,
            delay_per_attempt.clone(),
        )
        .await;

        // Alice gets the links from a different zome with remote call (same cell)
        let invocation = new_invocation(
            &alice_call_data.cell_id,
            "get_links_from_other_zome",
            alice_call_data.cell_id.agent_pubkey().clone(),
            TestWasm::CallRemoteCaller,
        )
        .unwrap();

        let links: Links = call(&handle, invocation).await;

        assert_eq!(links.into_inner().len(), 1);

        // Bob gets the links from a different zome in the same dna via remote call (different cell)
        let invocation = new_invocation(
            &bob_call_data.cell_id,
            "get_links_from_my_other_zome",
            (),
            TestWasm::CallRemoteCaller,
        )
        .unwrap();

        let links: Links = call(&handle, invocation).await;

        assert_eq!(links.into_inner().len(), 1);

        // Bob gets the links from a different zome with remote call (different cell)
        let invocation = new_invocation(
            &bob_call_data.cell_id,
            "get_links_from_other_zome",
            alice_call_data.cell_id.agent_pubkey().clone(),
            TestWasm::CallRemoteCaller,
        )
        .unwrap();

        let links: Links = call(&handle, invocation).await;

        assert_eq!(links.into_inner().len(), 1);

        // Bob getting links from the same zome (but different cell)
        let invocation = new_invocation(
            &bob_call_data.cell_id,
            "get_links_on_foo",
            (),
            TestWasm::CallRemoteCallee,
        )
        .unwrap();

        let links: Links = call(&handle, invocation).await;

        assert_eq!(links.into_inner().len(), 1);

        ConductorTestData::shutdown_conductor(handle).await;
    }

    async fn call<T: TryFrom<SerializedBytes>>(
        handle: &ConductorHandle,
        invocation: ZomeCallInvocation,
    ) -> T
    where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>,
    {
        let out = handle.call_zome(invocation).await.unwrap().unwrap();
        unwrap_to::unwrap_to!(out => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap()
    }
}
