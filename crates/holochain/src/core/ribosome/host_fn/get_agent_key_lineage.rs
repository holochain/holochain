use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_agent_key_lineage<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AgentPubKey,
) -> Result<Vec<AgentPubKey>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            agent_info: Permission::Allow,
            ..
        } => match call_context.host_context.maybe_dpki() {
            // If DPKI is not installed, agents cannot update keys. The lineage is just the one agent key.
            None => Ok(vec![input]),
            Some(dpki) => tokio_helper::block_forever_on(async move {
                let state = dpki.state().await;
                state
                    .get_agent_key_lineage(input)
                    .await
                    .map_err(|error| RuntimeError::new(error.to_string()))
            }),
        },
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_agent_key_lineage".into()
            )
            .to_string()
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn host_fn_get_agent_key_lineage() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::AgentKeyLineage).await;

        let agent_key_lineage: Vec<AgentPubKey> = conductor
            .call(&alice, "get_lineage_of_agent_keys", alice_pubkey.clone())
            .await;
        assert_eq!(agent_key_lineage, vec![alice_pubkey]);

        // TODO: When adding a function to update an agent key to DPKI service, append to this test
        // a key update and make sure it's included in the key lineage in the correct order.
    }
}
