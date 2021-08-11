use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use holochain_keystore::KeystoreSenderExt;
use tracing::error;

#[allow(clippy::extra_unused_lifetimes)]
pub fn accept_countersigning_preflight_request<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: PreflightRequest,
) -> Result<PreflightRequestAcceptance, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ agent_info: Permission::Allow, keystore: Permission::Allow, non_determinism: Permission::Allow, .. } => {
            if let Err(e) = input.check_integrity() {
                return Ok(PreflightRequestAcceptance::Invalid(e.to_string()));
            }
            tokio_helper::block_forever_on(async move {
                if holochain_types::timestamp::now().0 + SESSION_TIME_FUTURE_MAX_MILLIS < input.session_times().start().0
                {
                    return Ok(PreflightRequestAcceptance::UnacceptableFutureStart);
                }

                let author = call_context.host_context.workspace().source_chain().agent_pubkey().clone();
                let agent_index = match input
                .signing_agents()
                .iter()
                .position(|(agent, _)| agent == &author)
                {
                    Some(agent_index) => agent_index as u8,
                    None => return Ok(PreflightRequestAcceptance::UnacceptableAgentNotFound),
                };
                let countersigning_agent_state = call_context.host_context.workspace().source_chain().accept_countersigning_preflight_request(input.clone(), agent_index).await.map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
                let signature: Signature = match call_context
                    .host_context
                    .keystore()
                    .sign(Sign::new_raw(
                        author,
                        PreflightResponse::encode_fields_for_signature(&input, &countersigning_agent_state)?,
                    ))
                    .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        // Attempt to unlock the chain again.
                        // If this fails the chain will remain locked until the session end time.
                        // But also we're handling a keystore error already so we should return that.
                        if let Err(unlock_result) = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .unlock_chain()
                            .await {
                                error!(?unlock_result);
                            }
                        return Err(WasmError::Host(e.to_string()));
                    }
                };

                Ok(PreflightRequestAcceptance::Accepted(PreflightResponse::try_new(
                    input,
                    countersigning_agent_state,
                    signature,
                ).map_err(|e| WasmError::Host(e.to_string()))?))
            })
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use ::fixt::prelude::*;
    use holochain_types::prelude::AgentPubKeyFixturator;
    use holochain_zome_types::prelude::*;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use holochain_wasm_test_utils::TestWasm;
    use hdk::prelude::*;
    use crate::conductor::{api::ZomeCall};
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;
    use crate::conductor::api::error::ConductorApiError;
    use crate::core::workflow::error::WorkflowError;
    use holochain_state::source_chain::SourceChainError;
    use crate::conductor::CellError;
    use crate::core::ribosome::error::RibosomeError;

    #[tokio::test(flavor = "multi_thread")]
    async fn new_preflight_request() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let alice = fixt!(AgentPubKey, Predictable, 0);
        let bob = fixt!(AgentPubKey, Predictable, 1);
        let output: PreflightRequest = crate::call_test_ribosome!(
            host_access,
            TestWasm::CounterSigning,
            "generate_countersigning_preflight_request",
            vec![(alice, vec![Role(0)]), (bob, vec![])]
        ).unwrap();

        dbg!(&output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn accept_preflight() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let alice = fixt!(AgentPubKey, Predictable, 0);
        let bob = fixt!(AgentPubKey, Predictable, 1);
        let preflight_request: PreflightRequest = crate::call_test_ribosome!(
            host_access,
            TestWasm::CounterSigning,
            "generate_countersigning_preflight_request",
            vec![(alice, vec![Role(0)]), (bob, vec![])]
        ).unwrap();

        let acceptance: PreflightRequestAcceptance = crate::call_test_ribosome!(
            host_access,
            TestWasm::CounterSigning,
            "accept_countersigning_preflight_request",
            preflight_request
        ).unwrap();

        dbg!(&acceptance);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lock_chain() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store.expect_get().return_const(Some(dna_file.clone().into()));
        dna_store.expect_get_entry_def().return_const(EntryDef::default_with_id("thing"));

        let mut conductor = SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::CounterSigning);
        let bobbo = bobbo.zome(TestWasm::CounterSigning);

        // Before the preflight creation of things should work.
        let _: HeaderHash = conductor
            .call(
                &alice,
                "create_a_thing",
                (),
            ).await;

        // Alice can create multiple preflight requests.
        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![(alice_pubkey.clone(), vec![Role(0)]), (bob_pubkey.clone(), vec![])],
            )
            .await;
        let preflight_request_2: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![(alice_pubkey.clone(), vec![Role(1)]), (bob_pubkey.clone(), vec![])],
            )
            .await;

        // Alice can still create things before the preflight is accepted.
        let _: HeaderHash = conductor
            .call(
                &alice,
                "create_a_thing",
                (),
            ).await;

        // Alice can accept the preflight request.
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response = if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };

        // Can't accept a second preflight request while the first is active.
        let preflight_acceptance_fail = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: alice.cell_id().clone(),
                zome_name: alice.name().clone(),
                fn_name: "accept_countersigning_preflight_request".into(),
                cap: None,
                provenance: alice_pubkey.clone(),
                payload: ExternIO::encode(&preflight_request_2).unwrap(),
            }).await;
        assert!(
            matches!(
                preflight_acceptance_fail,
                Ok(Err(RibosomeError::WasmError(WasmError::Host(_))))
            )
        );

        // Bob can also accept the preflight request.
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bobbo,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
            response
        } else {
            unreachable!();
        };

        // With an accepted preflight creations must fail for alice.
        let thing_fail_create_alice = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: alice.cell_id().clone(),
                zome_name: alice.name().clone(),
                fn_name: "create_a_thing".into(),
                cap: None,
                provenance: alice_pubkey.clone(),
                payload: ExternIO::encode(()).unwrap(),
            }).await;
        match thing_fail_create_alice {
            Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => match *workflow_error {
                WorkflowError::SourceChainError(SourceChainError::ChainLocked) => { },
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        let thing_fail_create_bob = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: bobbo.cell_id().clone(),
                zome_name: bobbo.name().clone(),
                fn_name: "create_a_thing".into(),
                cap: None,
                provenance: bob_pubkey.clone(),
                payload: ExternIO::encode(()).unwrap(),
            }).await;
        match thing_fail_create_bob {
            Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => match *workflow_error {
                WorkflowError::SourceChainError(SourceChainError::ChainLocked) => { },
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        // Creating the correct countersigned entry will unlock the chain.
        let _: HeaderHash = conductor
            .call(
                &alice,
                "create_a_countersigned_thing",
                vec![alice_response.clone(), bob_response.clone()],
            ).await;

        let _: HeaderHash = conductor
            .call(
                &alice,
                "create_a_thing",
                (),
            ).await;

        // Creation will still fail for bob.
        let thing_fail_create_bob = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: bobbo.cell_id().clone(),
                zome_name: bobbo.name().clone(),
                fn_name: "create_a_thing".into(),
                cap: None,
                provenance: bob_pubkey.clone(),
                payload: ExternIO::encode(()).unwrap(),
            }).await;
        match thing_fail_create_bob {
            Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => match *workflow_error {
                WorkflowError::SourceChainError(SourceChainError::ChainLocked) => { },
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        // After bob commits the same countersigned entry he can unlock his chain.
        let _: HeaderHash = conductor
            .call(
                &bobbo,
                "create_a_countersigned_thing",
                vec![alice_response, bob_response],
            ).await;

        let _: HeaderHash = conductor
            .call(
                &bobbo,
                "create_a_thing",
                ()
            ).await;
    }

}