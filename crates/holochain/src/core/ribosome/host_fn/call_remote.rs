use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_wasmer_host::prelude::SerializedBytes;
use holochain_zome_types::CallRemoteInput;
use holochain_zome_types::CallRemoteOutput;
use std::sync::Arc;

// const CALL_REMOTE_TIMEOUT: u64 = 10_000;

pub fn call_remote(
    _ribosome: Arc<WasmRibosome>,
    call_context: Arc<CallContext>,
    input: CallRemoteInput,
) -> RibosomeResult<CallRemoteOutput> {
    dbg!(&input);
    let result: SerializedBytes = tokio_safe_block_on::tokio_safe_block_forever_on(
        async move {
            let mut network = call_context.host_access().network().clone();
            let call_remote = input.into_inner();
            let response = network
                .call_remote(
                    call_remote.to_agent(),
                    call_remote.zome_name(),
                    call_remote.fn_name(),
                    call_remote.cap(),
                    call_remote.request(),
                )
                .await;
            dbg!(&response);
            response
        },
        // std::time::Duration::from_millis(CALL_REMOTE_TIMEOUT),
    )?;

    dbg!(&result);

    Ok(CallRemoteOutput::new(result))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::conductor::manager::spawn_task_manager;
    use crate::conductor::Cell;
    use crate::core::ribosome::ZomeCallInvocation;
    use holochain_p2p::actor::HolochainP2pRefToCell;
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::test_utils::test_conductor_env;
    use holochain_state::test_utils::TestEnvironment;
    use holochain_types::cell::CellId;
    use holochain_types::dna::DnaDef;
    use holochain_types::dna::DnaFile;
    use holochain_wasm_test_utils::TestWasm;
    pub use holochain_zome_types::capability::CapSecret;
    use holochain_zome_types::HostInput;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    /// we can call a fn on a remote
    async fn call_remote_test() {
        // ////////////
        // START SHARED
        // ////////////

        let TestEnvironment { env, tmpdir } = test_conductor_env();
        let keystore = env.keystore().clone();
        let (holochain_p2p, _p2p_evt) = holochain_p2p::spawn_holochain_p2p().await.unwrap();

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
        // let mut agent_id_fixturator = holo_hash::fixt::AgentPubKeyFixturator::new(fixt::Unpredictable);

        let path = tmpdir.path().to_path_buf();

        let mut mock_handler = crate::conductor::handle::mock::MockConductorHandle::new();
        // let mock_dna = dna_file.clone();
        let _ = mock_handler
            .expect_sync_get_dna()
            .return_const(Some(dna_file.clone()));

        let mock_handler: crate::conductor::handle::ConductorHandle = Arc::new(mock_handler);

        let (add_task_sender, shutdown) = spawn_task_manager();
        let (stop_tx, _) = tokio::sync::broadcast::channel(1);

        // //////////
        // END SHARED
        // //////////

        // ///////////
        // START ALICE
        // ///////////

        let alice_agent_id = holo_hash::AgentPubKey::try_from(
            "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
        )
        .unwrap();
        let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
        let alice_holochain_p2p_cell =
            holochain_p2p.to_cell(dna_file.dna_hash().to_owned(), alice_agent_id.clone());

        Cell::genesis(
            alice_cell_id.clone(),
            mock_handler.clone(),
            path.clone(),
            keystore.clone(),
            None,
        )
        .await
        .unwrap();

        let alice_cell = Cell::create(
            alice_cell_id.clone(),
            mock_handler.clone(),
            path.clone(),
            keystore.clone(),
            alice_holochain_p2p_cell.clone(),
            add_task_sender.clone(),
            stop_tx.clone(),
        )
        .await
        .unwrap();

        // /////////
        // END ALICE
        // /////////

        // /////////
        // START BOB
        // /////////

        let bob_agent_id = holo_hash::AgentPubKey::try_from(
            "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
        )
        .unwrap();
        let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
        let bob_holochain_p2p_cell =
            holochain_p2p.to_cell(dna_file.dna_hash().to_owned(), bob_agent_id.clone());

        Cell::genesis(
            bob_cell_id.clone(),
            mock_handler.clone(),
            path.clone(),
            keystore.clone(),
            None,
        )
        .await
        .unwrap();

        let _bob_cell = Cell::create(
            bob_cell_id,
            mock_handler.clone(),
            path.clone(),
            keystore.clone(),
            bob_holochain_p2p_cell.clone(),
            add_task_sender.clone(),
            stop_tx.clone(),
        )
        .await
        .unwrap();

        // ///////
        // END BOB
        // ///////

        // ALICE DOING A CALL

        let output = alice_cell
            .call_zome(ZomeCallInvocation {
                cell_id: alice_cell_id,
                zome_name: TestWasm::WhoAmI.into(),
                cap: CapSecret::default(),
                fn_name: "whoarethey".to_string(),
                payload: HostInput::new(bob_agent_id.try_into().unwrap()),
                provenance: alice_agent_id,
            })
            .await
            .unwrap()
            .unwrap();

        // should output bob's agent info
        dbg!(&output);

        // assert_eq!(
        //     output.into_inner(),
        //     bob_agent_id,
        // );

        stop_tx.send(()).unwrap();
        shutdown.await.unwrap();
    }
}
