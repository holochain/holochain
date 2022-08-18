use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn must_get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetAgentActivityInput,
) -> Result<Vec<RegisterAgentActivity>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let MustGetAgentActivityInput {
                author,
                chain_filter,
            } = input;

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = match call_context.host_context {
                    HostContext::Validate(_) => Cascade::from_workspace(workspace.stores(), None),
                    _ => Cascade::from_workspace_network(
                        &workspace,
                        call_context.host_context.network().clone(),
                    ),
                };
                let result = cascade
                    .must_get_agent_activity(author.clone(), chain_filter.clone())
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })?;

                use MustGetAgentActivityResponse::*;

                let result: Result<_, RuntimeError> = match (result, &call_context.host_context) {
                    (Activity(activity), _) => Ok(activity),
                    (IncompleteChain | ActionNotFound(_), HostContext::Init(_)) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                        ))
                        .into())
                    }
                    (IncompleteChain | ActionNotFound(_), HostContext::Validate(_)) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(ValidateCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                        ))
                        .into())
                    }
                    (IncompleteChain, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain is incomplete for author {} and filter {:?}",
                        author, chain_filter
                    )))
                    .into()),
                    (ActionNotFound(missing_action), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity is missing action {} for author {} and filter {:?}",
                        missing_action, author, chain_filter
                    )))
                    .into()),
                    (PositionNotHighest, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain has produced an invalid range because the top of the chain is not the highest action sequence for author {} and filter {:?}",
                        author, chain_filter
                    )))
                    .into()),
                    (EmptyRange, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain has produced an invalid range because the range is empty for author {} and filter {:?}",
                        author, chain_filter
                    )))
                    .into()),
                };

                result
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_agent_activity".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
pub mod test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        let a: ActionHash = conductor
            .call(&bob, "commit_something", Something(vec![1]))
            .await;

        let b: ActionHash = conductor
            .call(&bob, "commit_something", Something(vec![2]))
            .await;

        let c: ActionHash = conductor
            .call(&bob, "commit_something", Something(vec![3]))
            .await;

        let filter = ChainFilter::new(a.clone());

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain",
                (bob.cell_id().agent_pubkey().clone(), filter.clone()),
            )
            .await;

        // Try the same filter but on alice's chain.
        // This will fail because alice does not have `a` hash in her chain.
        let err: Result<ActionHash, _> = conductor
            .call_fallible(
                &alice,
                "commit_require_agents_chain",
                (alice.cell_id().agent_pubkey().clone(), filter),
            )
            .await;

        err.unwrap_err();

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain_recursive",
                (bob.cell_id().agent_pubkey().clone(), c.clone()),
            )
            .await;

        for i in 3..30 {
            let _: ActionHash = conductor
                .call(&bob, "commit_something", Something(vec![i]))
                .await;
        }

        let d: ActionHash = conductor
            .call(&bob, "commit_something", Something(vec![21]))
            .await;

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain_recursive",
                (bob.cell_id().agent_pubkey().clone(), d.clone()),
            )
            .await;

        let filter = ChainFilter::new(c.clone()).until(a.clone());

        let r: Vec<RegisterAgentActivity> = conductor
            .call(
                &alice,
                "call_must_get_agent_activity",
                (bob.cell_id().agent_pubkey().clone(), filter.clone()),
            )
            .await;

        assert_eq!(
            r.into_iter()
                .map(|op| op.action.hashed.hash)
                .collect::<Vec<_>>(),
            vec![c, b, a]
        )
    }
}
