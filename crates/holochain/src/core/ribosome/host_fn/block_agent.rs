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
    use crate::sweettest::*;
    use crate::test_utils::RibosomeTestFixture;
    use holo_hash::ActionHash;
    use holo_hash::AgentPubKey;
    use holochain_state::block::get_all_cell_blocks;
    use holochain_types::prelude::CapSecret;
    use holochain_types::prelude::ZomeCallResponse;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::block::{BlockTarget, CellBlockReason};

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
    async fn hdk_cell_block_adds_block_to_blockspan_database() {
        holochain_trace::test_run();
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("", [&dna_file]).await.unwrap();

        let blocks = conductor
            .spaces
            .conductor_db
            .test_read(|txn| get_all_cell_blocks(txn));
        assert!(blocks.is_empty());

        let agent_key = app.agent().clone();
        let cell_id = app.cells()[0].cell_id().clone();
        let zome = app.cells()[0].zome(TestWasm::Create);
        let _: () = conductor.call(&zome, "block_agent", agent_key).await;

        let blocks = conductor
            .spaces
            .conductor_db
            .test_read(|txn| get_all_cell_blocks(txn));

        let expected_cell_block_reason = CellBlockReason::App(vec![]);
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(blocks[0].target(), BlockTarget::Cell(id, reason) if *id == cell_id && *reason == expected_cell_block_reason)
        );
        assert_eq!(blocks[0].start(), holochain_timestamp::Timestamp::MIN);
        assert_eq!(blocks[0].end(), holochain_timestamp::Timestamp::MAX);
    }
}
