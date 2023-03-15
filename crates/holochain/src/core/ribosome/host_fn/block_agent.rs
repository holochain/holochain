use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTarget;
use holochain_zome_types::block::CellBlockReason;
use holochain_types::prelude::*;

pub fn block_agent(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: holochain_zome_types::block::BlockAgentInput,
) -> Result<(), RuntimeError> {
    tokio_helper::block_forever_on(async move {
        call_context.host_context().call_zome_handle().block(Block::new(
            BlockTarget::Cell(CellId::new(call_context
                .host_context()
                .call_zome_handle()
                .cell_id().dna_hash()
                .clone(), input.target), CellBlockReason::App(input.reason)),
            input.interval
        )).await.map_err(|e| -> RuntimeError {
            wasm_error!(e.to_string()).into()
        })
    })
}

#[cfg(test)]
mod test {
    use holochain_types::prelude::CapSecret;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use holochain_wasm_test_utils::TestWasm;
    use holo_hash::AgentPubKey;
    use holochain_types::prelude::ZomeCallResponse;
    use holo_hash::ActionHash;
    use crate::conductor::api::error::ConductorApiResult;

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    pub struct CapFor(CapSecret, AgentPubKey);

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_call_verify_block() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, alice_pubkey, bob, bob_pubkey, ..
        } = RibosomeTestFixture::new(TestWasm::Capability).await;

        let secret: CapSecret = conductor.call(&bob, "cap_secret", ()).await;
        let _action_hash: ActionHash = conductor.call(&bob, "transferable_cap_grant", secret.clone()).await;
        let cap_for = CapFor(secret, bob_pubkey);
        let _response0: ZomeCallResponse = conductor.call(&alice, "try_cap_claim", cap_for.clone()).await;
        let _response1: ZomeCallResponse = conductor.call(&alice, "try_cap_claim", cap_for.clone()).await;

        let _: () = conductor.call(&bob, "block_agent", alice_pubkey.clone()).await;

        let response2: ConductorApiResult<ZomeCallResponse> = conductor.call_fallible(&alice, "try_cap_claim", cap_for.clone()).await;
        assert!(response2.is_err());

        let _: () = conductor.call(&bob, "unblock_agent", alice_pubkey).await;

        let _response3: ZomeCallResponse = conductor.call(&alice, "try_cap_claim", cap_for).await;
    }

}