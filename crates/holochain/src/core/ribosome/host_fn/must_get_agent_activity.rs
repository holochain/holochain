use crate::core::ribosome::host_fn::cascade_from_call_context;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::NetworkRequestOptions;
use holochain_state::mutations::insert_op_cache;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(_ribosome, call_context))
)]
pub fn must_get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetAgentActivityInput,
) -> Result<Vec<RegisterAgentActivity>, RuntimeError> {
    tracing::debug!("begin must_get_agent_activity");
    let ret = match HostFnAccess::from(&call_context.host_context()) {
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
                use crate::core::ribosome::ValidateHostAccess;
                let cascade = match call_context.host_context {
                    HostContext::Validate(ValidateHostAccess { is_inline, .. }) => {
                        if is_inline {
                            cascade_from_call_context(&call_context)
                        } else {
                            CascadeImpl::from_workspace_stores(workspace.stores(), None)
                                .with_zome_call_origin(
                                    call_context.zome.zome_name(),
                                    call_context.function_name(),
                                )
                        }
                    }
                    _ => cascade_from_call_context(&call_context),
                };
                let result = cascade
                    .must_get_agent_activity(
                        author.clone(),
                        chain_filter.clone(),
                        NetworkRequestOptions::must_get_options(),
                    )
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })?;

                use MustGetAgentActivityResponse::*;

                let result: Result<_, RuntimeError> = match (result, &call_context.host_context) {
                    (Activity {activity, warrants}, _) => {
                        if !warrants.is_empty() {
                            if let Some(db) = cascade.cache() {
                                db.write_async(|txn| {
                                    for warrant in warrants {
                                        insert_op_cache(txn, &DhtOpHashed::from_content_sync(warrant))?;
                                    }
                                    crate::conductor::error::ConductorResult::Ok(())
                                }).await.map_err(|e| -> RuntimeError { wasm_error!(e).into() })?;
                            }
                        }
                        Ok(activity)},
                    (
                        IncompleteChain
                        | ChainTopNotFound(_)
                        | UntilHashMissing(_)
                        | UntilTimestampIndeterminate(_),
                        HostContext::Init(_),
                    ) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into())
                    }
                    (
                        IncompleteChain
                        | ChainTopNotFound(_)
                        | UntilHashMissing(_)
                        | UntilTimestampIndeterminate(_),
                        HostContext::Validate(_),
                    ) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(ValidateCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into())
                    }
                    (IncompleteChain, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain is incomplete for author {author} and filter {chain_filter:?}"
                    )))
                    .into()),
                    (ChainTopNotFound(missing_action), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity is missing action {missing_action} for author {author} and filter {chain_filter:?}"
                    )))
                    .into()),
                    (UntilHashMissing(missing_action), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity is missing until hash {missing_action} for author {author} and filter {chain_filter:?}"
                    )))
                    .into()),
                    (UntilTimestampIndeterminate(missing_timestamp), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity is missing until timestamp {missing_timestamp} for author {author} and filter {chain_filter:?}"
                    )))
                    .into()),
                    (UntilHashAfterChainHead(until_hash), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity until_hash {until_hash} has action sequence after chain_top for author {author} and filter {chain_filter:?}"
                    )))
                    .into()),
                    (UntilTimestampGreaterThanChainHead(until_timestamp), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity until_timestamp {until_timestamp} is greater than chain_top timestamp for author {author} and filter {chain_filter:?}"
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
    };
    tracing::debug!(?ret);
    ret
}

#[cfg(test)]
pub mod test {
    use crate::test_utils::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    /// Test that validation can get the currently-being-validated agent's
    /// activity.
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity_self() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // This test is a repro of some issue where the init being inline with
        // the commit being validated may or may not be important. For that
        // reason this test should not be merged with other tests/assertions.
        let _: () = conductor
            .call(&alice, "commit_require_self_agents_chain", ())
            .await;
    }

    /// Test that validation can get the currently-being-validated agent's
    /// previous action bounded activity.
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity_self_prev() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // This test is a repro of some issue where the init being inline with
        // the commit being validated may or may not be important. For that
        // reason this test should not be merged with other tests/assertions.
        let _: () = conductor
            .call(&alice, "commit_require_self_prev_agents_chain", ())
            .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity() {
        holochain_trace::test_run();
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

        // Give bob time to integrate
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain_recursive",
                (bob.cell_id().agent_pubkey().clone(), d.clone()),
            )
            .await;

        let filter = ChainFilter::until_hash(c.clone(), a.clone());

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
