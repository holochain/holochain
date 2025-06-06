use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTarget;
use holochain_zome_types::block::CellBlockReason;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn block_agent(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: holochain_zome_types::block::BlockAgentInput,
) -> Result<(), RuntimeError> {
    tokio_helper::block_forever_on(async move {
        call_context
            .host_context()
            .call_zome_handle()
            .block(Block::new(
                BlockTarget::Cell(
                    CellId::new(
                        call_context
                            .host_context()
                            .call_zome_handle()
                            .cell_id()
                            .dna_hash()
                            .clone(),
                        input.target,
                    ),
                    CellBlockReason::App(input.reason),
                ),
                input.interval,
            ))
            .await
            .map_err(|e| -> RuntimeError { wasm_error!(e.to_string()).into() })
    })
}

#[cfg(test)]
mod test {
    use crate::conductor::api::error::ConductorApiResult;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::sweettest::*;
    use crate::test_utils::conditional_consistency::*;
    use holo_hash::ActionHash;
    use holo_hash::AgentPubKey;
    use holochain_types::prelude::CapSecret;
    use holochain_types::prelude::Record;
    use holochain_types::prelude::ZomeCallResponse;
    use holochain_wasm_test_utils::TestWasm;

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    pub struct CapFor(CapSecret, AgentPubKey);

    #[cfg(feature = "unstable-functions")]
    #[tokio::test(flavor = "multi_thread")]
    async fn zome_call_verify_block() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Capability).await;

        let secret: CapSecret = conductor.call(&bob, "cap_secret", ()).await;
        let _action_hash: ActionHash = conductor.call(&bob, "transferable_cap_grant", secret).await;
        let cap_for = CapFor(secret, bob_pubkey);
        let _response0: ZomeCallResponse = conductor
            .call(&alice, "try_cap_claim", cap_for.clone())
            .await;
        let _response1: ZomeCallResponse = conductor
            .call(&alice, "try_cap_claim", cap_for.clone())
            .await;

        let _: () = conductor
            .call(&bob, "block_agent", alice_pubkey.clone())
            .await;

        let response2: ConductorApiResult<ZomeCallResponse> = conductor
            .call_fallible(&alice, "try_cap_claim", cap_for.clone())
            .await;
        assert!(response2.is_err());

        let _: () = conductor.call(&bob, "unblock_agent", alice_pubkey).await;

        let _response3: ZomeCallResponse = conductor.call(&alice, "try_cap_claim", cap_for).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    async fn zome_call_get_block() {
        holochain_trace::test_run();

        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

        let config = SweetConductorConfig::standard()
            .tune_conductor(|c| {
                c.min_publish_interval = Some(std::time::Duration::from_secs(2));
                c.publish_trigger_interval = Some(std::time::Duration::from_secs(3));
                c.sys_validation_retry_delay = Some(std::time::Duration::from_secs(1));
            })
            .tune_network_config(|nc| {
                // Gossip would share peer info
                nc.disable_gossip = true;
                // Want to control the sharing of peer info in this test
                nc.disable_bootstrap = true;
            });
        let mut conductors = SweetConductorBatch::from_config(3, config).await;
        let apps = conductors.setup_app("create", [&dna_file]).await.unwrap();

        let ((alice_cell,), (bob_cell,), (carol_cell,)) = apps.into_tuples();

        conductors[0]
            .declare_full_storage_arcs(alice_cell.dna_hash())
            .await;
        conductors[1]
            .declare_full_storage_arcs(alice_cell.dna_hash())
            .await;
        conductors[2]
            .declare_full_storage_arcs(alice_cell.dna_hash())
            .await;

        let alice = alice_cell.zome(TestWasm::Create);
        let bob = bob_cell.zome(TestWasm::Create);

        let bob_pubkey = bob_cell.cell_id().agent_pubkey();

        conductors.reveal_peer_info(0, 1).await;
        conductors.reveal_peer_info(1, 0).await;

        let alice_conductor = conductors.get(0).unwrap();
        let bob_conductor = conductors.get(1).unwrap();

        let action0: ActionHash = alice_conductor.call(&alice, "create_entry", ()).await;

        await_consistency(30, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Before bob is blocked he can get posts just fine.
        let bob_get0: Option<Record> = bob_conductor.call(&bob, "get_post", action0).await;
        // Await bob's init to propagate to alice.
        await_consistency(30, [&alice_cell, &bob_cell])
            .await
            .unwrap();
        assert!(bob_get0.is_some());

        // Bob gets blocked by alice.
        let _block: () = alice_conductor
            .call(&alice, "block_agent", bob_pubkey)
            .await;

        let action1: ActionHash = alice_conductor.call(&alice, "create_entry", ()).await;

        // Now that bob is blocked by alice he cannot get data from alice.
        await_conditional_consistency(10, vec![], [(&alice_cell, true), (&bob_cell, false)])
            .await
            .unwrap();
        let bob_get1: Option<Record> = bob_conductor.call(&bob, "get_post", action1.clone()).await;

        assert!(bob_get1.is_none());

        // If carol joins the party but DOES NOT block bob then she will
        // give access to data once more for bob.

        conductors.exchange_peer_info().await;

        await_consistency(60, [&alice_cell, &bob_cell, &carol_cell])
            .await
            .unwrap();

        // Bob can get data from alice via. carol.
        let bob_get2: Option<Record> = bob_conductor.call(&bob, "get_post", action1).await;
        assert!(bob_get2.is_some());
    }
}
