use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::sys_validate::check_countersigning_preflight_request;
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
            if let Err(e) = check_countersigning_preflight_request(&input) {
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
                        dbg!("foo", &e);
                        return Err(WasmError::Host(e.to_string()));
                    }
                };

                Ok(PreflightRequestAcceptance::Accepted(PreflightResponse::new(
                    input,
                    countersigning_agent_state,
                    signature,
                )))
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
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;
    use hdk::prelude::*;
    use crate::core::ribosome::RibosomeError;
    use crate::test_utils::conductor_setup::ConductorTestData;
    use crate::conductor::{api::ZomeCall};

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
    async fn lock_chain_2() {
        observability::test_run().ok();

        let alice = fixt!(AgentPubKey, Predictable, 0);
        let bob = fixt!(AgentPubKey, Predictable, 1);

        dbg!("alice", &alice);
        dbg!("bob", &bob);

        let zomes = vec![TestWasm::CounterSigning];
        let conductor = ConductorTestData::two_agents(zomes, true).await;
        let alice_cell_id = conductor.alice_call_data().cell_id.clone();
        let bob_cell_id = conductor.bob_call_data().unwrap().cell_id.clone();

        let thing_create = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "create_a_thing".into(),
            payload: ExternIO::encode(()).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone(),
        })
        .await;
        dbg!(&thing_create);

        let preflight_request: PreflightRequest = if let ZomeCallResponse::Ok(response) = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "generate_countersigning_preflight_request".into(),
            payload: ExternIO::encode(vec![(alice, vec![Role(0)]), (bob, vec![])]).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone(),
        })
        .await.unwrap().unwrap() {
            ExternIO::decode(&response).unwrap()
        } else {
            unreachable!();
        };
        dbg!(&preflight_request);

        let thing_create_2 = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "create_a_thing".into(),
            payload: ExternIO::encode(()).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone(),
        })
        .await;
        dbg!(&thing_create_2);

        let alice_acceptance: PreflightRequestAcceptance = if let ZomeCallResponse::Ok(response) = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "accept_countersigning_preflight_request".into(),
            payload: ExternIO::encode(preflight_request.clone()).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone(),
        }).await.unwrap().unwrap() {
            ExternIO::decode(&response).unwrap()
        } else {
            unreachable!();
        };
        dbg!(&alice_acceptance);
        let alice_response = match alice_acceptance {
            PreflightRequestAcceptance::Accepted(response) => response,
            _ => unreachable!(),
        };

        let bob_acceptance: PreflightRequestAcceptance = if let ZomeCallResponse::Ok(response) = conductor.handle().call_zome(ZomeCall {
            cell_id: bob_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "accept_countersigning_preflight_request".into(),
            payload: ExternIO::encode(preflight_request.clone()).unwrap(),
            provenance: bob_cell_id.agent_pubkey().clone(),
        }).await.unwrap().unwrap() {
            ExternIO::decode(&response).unwrap()
        } else {
            unreachable!();
        };
        dbg!(&bob_acceptance);
        let bob_response = match bob_acceptance {
            PreflightRequestAcceptance::Accepted(response) => response,
            _ => unreachable!(),
        };

        let thing_fail_create = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "create_a_thing".into(),
            payload: ExternIO::encode(()).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone()
        }).await;
        dbg!(&thing_fail_create);

        let countersigned_thing: HeaderHash = if let ZomeCallResponse::Ok(response) = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "create_a_countersigned_thing".into(),
            payload: ExternIO::encode(vec![alice_response, bob_response]).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone()
        }).await.unwrap().unwrap() {
            ExternIO::decode(&response).unwrap()
        } else {
            unreachable!();
        };
        dbg!(&countersigned_thing);

        let thing_create_3 = conductor.handle().call_zome(ZomeCall {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::CounterSigning.into(),
            cap: None,
            fn_name: "create_a_thing".into(),
            payload: ExternIO::encode(()).unwrap(),
            provenance: alice_cell_id.agent_pubkey().clone(),
        })
        .await.unwrap().unwrap();
        dbg!(&thing_create_3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lock_chain() {
        let alice = fixt!(AgentPubKey, Predictable, 0);
        let bob = fixt!(AgentPubKey, Predictable, 1);

        let alice_test_env = holochain_state::test_utils::test_cell_env();
        let alice_test_cache = holochain_state::test_utils::test_cache_env();
        let alice_env = alice_test_env.env();

        // let bob_test_env = holochain_state::test_utils::test_cell_env_2();
        // let bob_test_cache = holochain_state::test_utils::test_cache_env_2();
        // let bob_env = bob_test_env.env();

        crate::test_utils::fake_genesis(alice_env.clone()).await.unwrap();
        let alice_workspace = HostFnWorkspace::new(alice_env.clone(), alice_test_cache.env(), alice.clone()).await.unwrap();
        let mut alice_host_access = fixt!(ZomeCallHostAccess, Predictable, 0);
        alice_host_access.workspace = alice_workspace;

        // crate::test_utils::fake_genesis_for_agent(bob_env.clone(), bob.clone()).await.unwrap();
        // let bob_workspace = HostFnWorkspace::new(bob_env.clone(), bob_test_cache.env(), bob.clone()).await.unwrap();
        let bob_host_access = fixt!(ZomeCallHostAccess, Predictable, 1);
        // bob_host_access.workspace = bob_workspace;

        let thing_create: HeaderHash = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "create_a_thing",
            ()
        ).unwrap();
        dbg!(&thing_create);
        let preflight_request: PreflightRequest = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "generate_countersigning_preflight_request",
            vec![(alice, vec![Role(0)]), (bob, vec![])]
        ).unwrap();
        dbg!(&preflight_request);

        let thing_create_2: HeaderHash = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "create_a_thing",
            ()
        ).unwrap();
        dbg!(&thing_create_2);

        let alice_acceptance: PreflightRequestAcceptance = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "accept_countersigning_preflight_request",
            preflight_request
        ).unwrap();
        dbg!(&alice_acceptance);
        let alice_response = match alice_acceptance {
            PreflightRequestAcceptance::Accepted(response) => response,
            _ => unreachable!(),
        };

        let bob_acceptance: PreflightRequestAcceptance = crate::call_test_ribosome!(
            bob_host_access,
            TestWasm::CounterSigning,
            "accept_countersigning_preflight_request",
            preflight_request
        ).unwrap();
        dbg!(&bob_acceptance);
        let bob_response = match bob_acceptance {
            PreflightRequestAcceptance::Accepted(response) => response,
            _ => unreachable!(),
        };

        let thing_fail_create: Result<HeaderHash, RibosomeError> = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "create_a_thing",
            ()
        );

        dbg!(&thing_fail_create);

        let countersigned_thing: HeaderHash = crate::call_test_ribosome!(
            alice_host_access,
            TestWasm::CounterSigning,
            "create_a_countersigned_thing",
            vec![alice_response, bob_response]
        ).unwrap();
        dbg!(&countersigned_thing);

    }
}