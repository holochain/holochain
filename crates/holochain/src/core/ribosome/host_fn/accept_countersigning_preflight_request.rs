use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use tracing::error;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn accept_countersigning_preflight_request<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: PreflightRequest,
) -> Result<PreflightRequestAcceptance, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            agent_info: Permission::Allow,
            keystore: Permission::Allow,
            non_determinism: Permission::Allow,
            write_workspace: Permission::Allow,
            ..
        } => {
            if let Err(e) = input.check_integrity() {
                return Ok(PreflightRequestAcceptance::Invalid(e.to_string()));
            }
            let author = super::agent_info::agent_info(_ribosome, call_context.clone(), ())?
                .agent_latest_pubkey;
            tokio_helper::block_forever_on(async move {
                if (holochain_zome_types::prelude::Timestamp::now() + SESSION_TIME_FUTURE_MAX)
                    .unwrap_or(Timestamp::MAX)
                    < *input.session_times.start()
                {
                    return Ok(PreflightRequestAcceptance::UnacceptableFutureStart);
                }

                let agent_index = match input
                    .signing_agents
                    .iter()
                    .position(|(agent, _)| agent == &author)
                {
                    Some(agent_index) => agent_index as u8,
                    None => return Ok(PreflightRequestAcceptance::UnacceptableAgentNotFound),
                };
                let countersigning_agent_state = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .accept_countersigning_preflight_request(input.clone(), agent_index)
                    .await
                    .map_err(|source_chain_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(source_chain_error.to_string())).into()
                    })?;
                let signature: Signature = match call_context
                    .host_context
                    .keystore()
                    .sign(
                        author,
                        PreflightResponse::encode_fields_for_signature(
                            &input,
                            &countersigning_agent_state,
                        )
                        .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        .into(),
                    )
                    .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        // Attempt to unlock the chain again.
                        // If this fails the chain will remain locked until the session end time.
                        // But also we're handling a keystore error already so we should return that.
                        if let Err(unlock_result) = call_context
                            .host_context
                            .workspace_write()
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain if write_workspace access is given")
                            .unlock_chain()
                            .await
                        {
                            error!(?unlock_result);
                        }
                        return Err(wasm_error!(WasmErrorInner::Host(e.to_string())).into());
                    }
                };

                Ok(PreflightRequestAcceptance::Accepted(
                    PreflightResponse::try_new(input, countersigning_agent_state, signature)
                        .map_err(|e| -> RuntimeError {
                            wasm_error!(WasmErrorInner::Host(e.to_string())).into()
                        })?,
                ))
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "accept_countersigning_preflight_request".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::conductor::api::error::ConductorApiError;
    use crate::conductor::api::ZomeCall;
    use crate::conductor::CellError;
    use crate::core::ribosome::error::RibosomeError;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::workflow::WorkflowError;
    use crate::sweettest::SweetConductorBatch;
    use crate::sweettest::SweetDnaFile;
    use crate::test_utils::consistency_10s;
    use hdk::prelude::*;
    use holochain_state::source_chain::SourceChainError;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::zome_io::ZomeCallUnsigned;
    use wasmer::RuntimeError;

    /// Allow ChainLocked error, panic on anything else
    fn expect_chain_locked(
        result: Result<Result<ZomeCallResponse, RibosomeError>, ConductorApiError>,
    ) {
        match result {
            Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => {
                match *workflow_error {
                    WorkflowError::SourceChainError(SourceChainError::ChainLocked) => {}
                    _ => panic!("{:?}", workflow_error),
                }
            }
            something_else => panic!("{:?}", something_else),
        };
    }

    /// Allow LockExpired error, panic on anything else
    fn expect_chain_lock_expired<T>(result: Result<T, ConductorApiError>)
    where
        T: std::fmt::Debug,
    {
        match result {
            Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => {
                match *workflow_error {
                    WorkflowError::SourceChainError(SourceChainError::LockExpired) => {}
                    _ => panic!("{:?}", workflow_error),
                }
            }
            something_else => panic!("{:?}", something_else),
        };
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    async fn unlock_timeout_session() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::CounterSigning).await;

        // Before preflight everyone commits some stuff.
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;
        let _: ActionHash = conductor.call(&bob, "create_a_thing", ()).await;

        let alice_agent_activity_alice_observed_before: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let alice_agent_activity_bob_observed_before: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let bob_agent_activity_alice_observed_before: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let bob_agent_activity_bob_observed_before: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        // Everyone accepts a short lived session.
        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request_fast",
                vec![
                    (alice_pubkey.clone(), vec![Role(0)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!();
            };
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bob,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        // Alice commits the session entry.
        let (countersigned_action_hash_alice, countersigned_entry_hash_alice): (
            ActionHash,
            EntryHash,
        ) = conductor
            .call(
                &alice,
                "create_a_countersigned_thing_with_entry_hash",
                vec![alice_response.clone(), bob_response.clone()],
            )
            .await;

        // Bob tries to do the same thing but after timeout.
        tokio::time::sleep(std::time::Duration::from_millis(10000)).await;
        let bob_result: Result<ActionHash, _> = conductor
            .call_fallible(
                &bob,
                "create_a_countersigned_thing",
                vec![alice_response.clone(), bob_response.clone()],
            )
            .await;
        expect_chain_lock_expired(bob_result);

        // At this point Alice's session entry is a liability so can't exist.
        let alice_agent_activity_alice_observed_after: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let alice_agent_activity_bob_observed_after: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let bob_agent_activity_alice_observed_after: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        let bob_agent_activity_bob_observed_after: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        assert_eq!(
            alice_agent_activity_alice_observed_before,
            alice_agent_activity_alice_observed_after
        );
        assert_eq!(
            alice_agent_activity_bob_observed_before,
            alice_agent_activity_bob_observed_after
        );
        assert_eq!(
            bob_agent_activity_alice_observed_before,
            bob_agent_activity_alice_observed_after
        );
        assert_eq!(
            bob_agent_activity_bob_observed_before,
            bob_agent_activity_bob_observed_after
        );

        // @TODO - the following all pass but perhaps we do NOT want them to?
        // It's not immediately clear what direct requests by hash should do in all cases here.
        //
        // If an author does a must_get during a zome call like we do in this test, should it
        // be returned (it's in the scratch ready to be flushed so it does atm) even though it
        // hasn't been countersigned and so may never be included in a source chain?
        //
        // Should it be returned in subsequent zome calls by an author who has signed it but it
        // hasn't been coauthored yet, but the session is still active? (c.f. private entries being visible to author)
        // What about after the session?
        //
        // What about returned by/for coauthors who do NOT sign during and after the session?
        //
        // What about everyone else during and after the session?
        //
        // The answer to the above may be different per call, idk at this point.
        // Seems intuitive that an action that is in nobody's agent activity should never be visible
        // but then how can you get the entry hash and entry data during the session, like we do in this test?
        //
        // Maybe it also seems intuitive that must_get_entry should return the entry as we know its
        // hash and normally must_get ignores validity or even which headers created it, but what if NO
        // headers created it?
        //
        // etc. etc. I'm just leaving this commentary here to germinate future headaches and self doubt.
        let _alice_action: SignedActionHashed = conductor
            .call(
                &alice,
                "must_get_action",
                countersigned_action_hash_alice.clone(),
            )
            .await;

        let _alice_record: Record = conductor
            .call(
                &alice,
                "must_get_valid_record",
                countersigned_action_hash_alice.clone(),
            )
            .await;
        let _alice_entry: EntryHashed = conductor
            .call(
                &alice,
                "must_get_entry",
                countersigned_entry_hash_alice.clone(),
            )
            .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    #[cfg_attr(target_os = "macos", ignore = "flaky")]
    async fn unlock_invalid_session() {
        use holochain_nonce::fresh_nonce;

        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::CounterSigning).await;
        let now = Timestamp::now();

        // Before preflight Alice can commit
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;

        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_invalid_countersigning_preflight_request",
                vec![
                    (alice_pubkey.clone(), vec![Role(0)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;

        // Before accepting preflight Alice can commit
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;

        // Alice can accept the preflight request.
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!();
            };

        // Bob can also accept the preflight request.
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bob,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // With an accepted preflight creations must fail for alice.
        let thing_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;

        expect_chain_locked(thing_fail_create_alice);

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // Creating the INCORRECT countersigned entry WILL immediately unlock
        // the chain.
        let countersign_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_an_invalid_countersigned_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(vec![
                            alice_response.clone(),
                            bob_response.clone(),
                        ])
                        .unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        assert!(matches!(countersign_fail_create_alice, Err(_)));
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    async fn lock_chain() {
        use holochain_nonce::fresh_nonce;
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_cell,
            alice_pubkey,
            bob,
            bob_cell,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::CounterSigning).await;
        let now = Timestamp::now();
        // Before the preflight creation of things should work.
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;

        // Bob's zome must be initialized for countersigning to work.
        let _: ActionHash = conductor.call(&bob, "create_a_thing", ()).await;

        // Alice can create a preflight request.
        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![
                    (alice_pubkey.clone(), vec![Role(0)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;

        // Alice can accept the preflight request.
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!();
            };

        // Alice can create a second preflight request.
        let preflight_request_2: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![
                    (alice_pubkey.clone(), vec![Role(1)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // Can't accept a second preflight request while the first is active.
        let preflight_acceptance_fail = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "accept_countersigning_preflight_request".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(&preflight_request_2).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        assert!(matches!(
            preflight_acceptance_fail,
            Ok(Err(RibosomeError::WasmRuntimeError(RuntimeError { .. })))
        ));

        // Bob can also accept the preflight request.
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bob,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // With an accepted preflight creations must fail for alice.
        let thing_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_alice);

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        let thing_fail_create_bob = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: bob.cell_id().clone(),
                        zome_name: bob.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: bob_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_bob);

        // Creating the correct countersigned entry will NOT immediately unlock
        // the chain (it needs Bob to countersign).
        let countersigned_action_hash_alice: ActionHash = conductor
            .call(
                &alice,
                "create_a_countersigned_thing",
                vec![alice_response.clone(), bob_response.clone()],
            )
            .await;

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        let thing_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        expect_chain_locked(thing_fail_create_alice);

        // The countersigned entry does NOT appear in alice's activity yet.
        let alice_activity_pre: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        assert_eq!(alice_activity_pre.valid_activity.len(), 5);

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // Creation will still fail for bob.
        let thing_fail_create_bob = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: bob.cell_id().clone(),
                        zome_name: bob.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: bob_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_bob);

        // After bob commits the same countersigned entry he can unlock his chain.
        let countersigned_action_hash_bob: ActionHash = conductor
            .call(
                &bob,
                "create_a_countersigned_thing",
                vec![alice_response, bob_response],
            )
            .await;
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;
        let _: ActionHash = conductor.call(&bob, "create_a_thing", ()).await;

        // Action get must not error.
        let countersigned_action_bob: SignedActionHashed = conductor
            .call(
                &bob,
                "must_get_action",
                countersigned_action_hash_bob.clone(),
            )
            .await;
        let countersigned_action_alice: SignedActionHashed = conductor
            .call(
                &alice,
                "must_get_action",
                countersigned_action_hash_alice.clone(),
            )
            .await;

        // Entry get must not error.
        if let Some((countersigned_entry_hash_bob, _)) =
            countersigned_action_bob.action().entry_data()
        {
            let _countersigned_entry_bob: EntryHashed = conductor
                .call(&bob, "must_get_entry", countersigned_entry_hash_bob)
                .await;
        } else {
            unreachable!();
        }

        // Record get must not error.
        let _countersigned_record_bob: Record = conductor
            .call(&bob, "must_get_valid_record", countersigned_action_hash_bob)
            .await;

        let alice_activity: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        consistency_10s([&alice_cell, &bob_cell]).await;

        assert_eq!(alice_activity.valid_activity.len(), 7);
        assert_eq!(
            &alice_activity.valid_activity[5].1,
            countersigned_action_alice.action_address(),
        );

        let bob_activity: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        assert_eq!(bob_activity.valid_activity.len(), 7);
        assert_eq!(
            &bob_activity.valid_activity[5].1,
            countersigned_action_bob.action_address(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    #[ignore = "countersigning_an_entry_before_bobs_zome_initialized_fails"]
    async fn lock_chain_failure() {
        use holochain_nonce::fresh_nonce;
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_cell,
            alice_pubkey,
            bob,
            bob_cell,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::CounterSigning).await;
        let now = Timestamp::now();
        // Before the preflight creation of things should work.
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;

        // Alice can create multiple preflight requests.
        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![
                    (alice_pubkey.clone(), vec![Role(0)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;
        let preflight_request_2: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request",
                vec![
                    (alice_pubkey.clone(), vec![Role(1)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;

        // Alice can still create things before the preflight is accepted.
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;

        // Alice can accept the preflight request.
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!();
            };

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // Can't accept a second preflight request while the first is active.
        let preflight_acceptance_fail = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "accept_countersigning_preflight_request".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(&preflight_request_2).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        assert!(matches!(
            preflight_acceptance_fail,
            Ok(Err(RibosomeError::WasmRuntimeError(RuntimeError { .. })))
        ));

        // Bob can also accept the preflight request.
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bob,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // With an accepted preflight creations must fail for alice.
        let thing_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_alice);

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        let thing_fail_create_bob = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: bob.cell_id().clone(),
                        zome_name: bob.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: bob_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_bob);

        // Creating the correct countersigned entry will NOT immediately unlock
        // the chain (it needs Bob to countersign).
        let countersigned_action_hash_alice: ActionHash = conductor
            .call(
                &alice,
                "create_a_countersigned_thing",
                vec![alice_response.clone(), bob_response.clone()],
            )
            .await;

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        let thing_fail_create_alice = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: alice.cell_id().clone(),
                        zome_name: alice.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: alice_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        expect_chain_locked(thing_fail_create_alice);

        // The countersigned entry does NOT appear in alice's activity yet.
        let alice_activity_pre: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        assert_eq!(alice_activity_pre.valid_activity.len(), 6);

        let (nonce, expires_at) = fresh_nonce(now).unwrap();

        // Creation will still fail for bob.
        let thing_fail_create_bob = conductor
            .raw_handle()
            .call_zome(
                ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: bob.cell_id().clone(),
                        zome_name: bob.name().clone(),
                        fn_name: "create_a_thing".into(),
                        cap_secret: None,
                        provenance: bob_pubkey.clone(),
                        payload: ExternIO::encode(()).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap(),
            )
            .await;
        expect_chain_locked(thing_fail_create_bob);

        // After bob commits the same countersigned entry he can unlock his chain.
        let countersigned_action_hash_bob: ActionHash = conductor
            .call(
                &bob,
                "create_a_countersigned_thing",
                vec![alice_response, bob_response],
            )
            .await;
        let _: ActionHash = conductor.call(&alice, "create_a_thing", ()).await;
        let _: ActionHash = conductor.call(&bob, "create_a_thing", ()).await;

        // Action get must not error.
        let countersigned_action_bob: SignedActionHashed = conductor
            .call(
                &bob,
                "must_get_action",
                countersigned_action_hash_bob.clone(),
            )
            .await;
        let countersigned_action_alice: SignedActionHashed = conductor
            .call(
                &alice,
                "must_get_action",
                countersigned_action_hash_alice.clone(),
            )
            .await;

        // Entry get must not error.
        if let Some((countersigned_entry_hash_bob, _)) =
            countersigned_action_bob.action().entry_data()
        {
            let _countersigned_entry_bob: EntryHashed = conductor
                .call(&bob, "must_get_entry", countersigned_entry_hash_bob)
                .await;
        } else {
            unreachable!();
        }

        // Record get must not error.
        let _countersigned_record_bob: Record = conductor
            .call(&bob, "must_get_valid_record", countersigned_action_hash_bob)
            .await;

        let alice_activity: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        consistency_10s([&alice_cell, &bob_cell]).await;

        assert_eq!(alice_activity.valid_activity.len(), 8);
        assert_eq!(
            &alice_activity.valid_activity[6].1,
            countersigned_action_alice.action_address(),
        );

        let bob_activity: AgentActivity = conductor
            .call(
                &bob,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        assert_eq!(bob_activity.valid_activity.len(), 6);
        assert_eq!(
            &bob_activity.valid_activity[4].1,
            countersigned_action_bob.action_address(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    #[ignore = "flaky"]
    /// TODO: this test and the following one (`enzymatic_session_success_forced_init`) form a pair.
    /// The latter includes a "fix" to the test to remove the flakiness, but the flakiness itself is a problem
    /// that we need to address.
    /// The flakiness is described in https://github.com/holochain/holochain/pull/3046. When that is resolved,
    /// this test can be unignored, and the companion test can be removed.
    async fn enzymatic_session_success_flaky() {
        enzymatic_session_success(false).await
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    async fn enzymatic_session_success_forced_init() {
        enzymatic_session_success(true).await
    }

    async fn enzymatic_session_success(force_init: bool) {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_cell,
            alice_pubkey,
            bob,
            bob_cell,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::CounterSigning).await;

        if force_init {
            // Run any arbitrary zome call for bob to force him to run init
            let _: AgentActivity = conductor
                .call(
                    &bob,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: bob_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;
        }

        // Start an enzymatic session
        let preflight_request: PreflightRequest = conductor
            .call(
                &alice,
                "generate_countersigning_preflight_request_enzymatic",
                vec![
                    // Alice is enzyme
                    (alice_pubkey.clone(), vec![Role(0)]),
                    (bob_pubkey.clone(), vec![]),
                ],
            )
            .await;

        // Alice can accept.
        let alice_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &alice,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!();
            };

        // Bob can also accept the preflight request.
        let bob_acceptance: PreflightRequestAcceptance = conductor
            .call(
                &bob,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await;
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        // Alice commits the action.
        let _countersigned_action_hash_alice: ActionHash = conductor
            .call(
                &alice,
                "create_a_countersigned_thing",
                vec![alice_response.clone(), bob_response.clone()],
            )
            .await;

        // The countersigned entry does NOT appear in alice's activity yet.
        let alice_activity_pre: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        // Nor bob's.
        let bob_activity_pre: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        // Bob commits the action also.
        let _countersigned_action_hash_bob: ActionHash = conductor
            .call(
                &bob,
                "create_a_countersigned_thing",
                vec![alice_response, bob_response],
            )
            .await;

        consistency_10s([&alice_cell, &bob_cell]).await;

        // Now the action appears in alice's activty.
        let alice_activity: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: alice_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;
        // And bob's.
        let bob_activity: AgentActivity = conductor
            .call(
                &alice,
                "get_agent_activity",
                GetAgentActivityInput {
                    agent_pubkey: bob_pubkey.clone(),
                    chain_query_filter: ChainQueryFilter::new(),
                    activity_request: ActivityRequest::Full,
                },
            )
            .await;

        assert_eq!(
            alice_activity.valid_activity.len(),
            alice_activity_pre.valid_activity.len() + 1
        );
        assert_eq!(
            bob_activity.valid_activity.len(),
            bob_activity_pre.valid_activity.len() + 1
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "slow_tests")]
    #[ignore = "countersigning_an_entry_before_bobs_zome_initialized_fails"]
    async fn enzymatic_session_failure() {
        holochain_trace::test_run().ok();

        let (dna_file, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;

        let mut conductors = SweetConductorBatch::from_standard_config(3).await;
        let apps = conductors
            .setup_app("countersigning", [&dna_file])
            .await
            .unwrap();

        let ((alice_cell,), (bob_cell,), (carol_cell,)) = apps.into_tuples();

        let alice = alice_cell.zome(TestWasm::CounterSigning);
        let bob = bob_cell.zome(TestWasm::CounterSigning);

        let alice_pubkey = alice_cell.cell_id().agent_pubkey();
        let bob_pubkey = bob_cell.cell_id().agent_pubkey();

        // Alice and bob can see carol but not each other.
        // We will simply teleport the countersigning requests and responses.
        conductors.reveal_peer_info(0, 2).await;
        conductors.reveal_peer_info(1, 2).await;

        let alice_conductor = conductors.get(0).unwrap();
        let bob_conductor = conductors.get(1).unwrap();

        // NON ENZYMATIC
        {
            consistency_10s([&alice_cell, &bob_cell, &carol_cell]).await;

            // The countersigned entry does NOT appear in alice's activity yet.
            let alice_activity_pre: AgentActivity = bob_conductor
                .call(
                    &bob,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: alice_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;
            // Nor bob's.
            let bob_activity_pre: AgentActivity = alice_conductor
                .call(
                    &alice,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: bob_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;

            // Start a session
            let preflight_request: PreflightRequest = alice_conductor
                .call(
                    &alice,
                    "generate_countersigning_preflight_request",
                    vec![
                        // Alice is enzyme
                        (alice_pubkey.clone(), vec![Role(0)]),
                        (bob_pubkey.clone(), vec![]),
                    ],
                )
                .await;

            // Alice can accept.
            let alice_acceptance: PreflightRequestAcceptance = alice_conductor
                .call(
                    &alice,
                    "accept_countersigning_preflight_request",
                    preflight_request.clone(),
                )
                .await;
            let alice_response =
                if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                    response
                } else {
                    unreachable!();
                };

            // Bob can also accept the preflight request.
            let bob_acceptance: PreflightRequestAcceptance = bob_conductor
                .call(
                    &bob,
                    "accept_countersigning_preflight_request",
                    preflight_request.clone(),
                )
                .await;
            let bob_response =
                if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                    response
                } else {
                    unreachable!();
                };

            consistency_10s([&alice_cell, &bob_cell, &carol_cell]).await;

            // Alice commits the action.
            let _countersigned_action_hash_alice: ActionHash = alice_conductor
                .call(
                    &alice,
                    "create_a_countersigned_thing",
                    vec![alice_response.clone(), bob_response.clone()],
                )
                .await;

            // Bob commits the action also.
            let _countersigned_action_hash_bob: ActionHash = bob_conductor
                .call(
                    &bob,
                    "create_a_countersigned_thing",
                    vec![alice_response, bob_response],
                )
                .await;

            consistency_10s([&alice_cell, &bob_cell, &carol_cell]).await;

            // Now the action appears in alice's activty.
            let alice_activity: AgentActivity = bob_conductor
                .call(
                    &bob,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: alice_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;

            // And bob's.
            let bob_activity: AgentActivity = alice_conductor
                .call(
                    &alice,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: bob_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;

            assert_eq!(
                alice_activity.valid_activity.len(),
                alice_activity_pre.valid_activity.len() + 2,
                "Expected alice's activity to have {} items but was {}, have got this activity {:?}",
                alice_activity_pre.valid_activity.len() + 2,
                alice_activity.valid_activity.len(),
                alice_activity,
            );
            assert_eq!(
                bob_activity.valid_activity.len(),
                bob_activity_pre.valid_activity.len() + 2,
                "Expected bob's activity to have {} items but was {}, have got this activity {:?}",
                bob_activity_pre.valid_activity.len() + 2,
                bob_activity.valid_activity.len(),
                bob_activity,
            );
        }

        // ENZYMATIC

        {
            // Start an enzymatic session
            let preflight_request: PreflightRequest = alice_conductor
                .call(
                    &alice,
                    "generate_countersigning_preflight_request_enzymatic",
                    vec![
                        // Alice is enzyme
                        (alice_pubkey.clone(), vec![Role(0)]),
                        (bob_pubkey.clone(), vec![]),
                    ],
                )
                .await;

            // Alice can accept.
            let alice_acceptance: PreflightRequestAcceptance = alice_conductor
                .call(
                    &alice,
                    "accept_countersigning_preflight_request",
                    preflight_request.clone(),
                )
                .await;
            let alice_response =
                if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                    response
                } else {
                    unreachable!();
                };

            // Bob can also accept the preflight request.
            let bob_acceptance: PreflightRequestAcceptance = bob_conductor
                .call(
                    &bob,
                    "accept_countersigning_preflight_request",
                    preflight_request.clone(),
                )
                .await;
            let bob_response =
                if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                    response
                } else {
                    unreachable!();
                };

            // Alice commits the action.
            let _countersigned_action_hash_alice: ActionHash = alice_conductor
                .call(
                    &alice,
                    "create_a_countersigned_thing",
                    vec![alice_response.clone(), bob_response.clone()],
                )
                .await;

            // The countersigned entry does NOT appear in alice's activity yet.
            let alice_activity_pre: AgentActivity = bob_conductor
                .call(
                    &bob,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: alice_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;
            // Nor bob's.
            let bob_activity_pre: AgentActivity = alice_conductor
                .call(
                    &alice,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: bob_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;

            // Bob commits the action also.
            let _countersigned_action_hash_bob: ActionHash = bob_conductor
                .call(
                    &bob,
                    "create_a_countersigned_thing",
                    vec![alice_response, bob_response],
                )
                .await;

            // Now the action DOES NOT appear in alice's activty, due to the
            // partition blocking the enzyme push.
            let alice_activity: AgentActivity = bob_conductor
                .call(
                    &bob,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: alice_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;
            // Same for bob's.
            let bob_activity: AgentActivity = alice_conductor
                .call(
                    &alice,
                    "get_agent_activity",
                    GetAgentActivityInput {
                        agent_pubkey: bob_pubkey.clone(),
                        chain_query_filter: ChainQueryFilter::new(),
                        activity_request: ActivityRequest::Full,
                    },
                )
                .await;

            assert_eq!(
                alice_activity.valid_activity.len(),
                alice_activity_pre.valid_activity.len()
            );
            assert_eq!(
                bob_activity.valid_activity.len(),
                bob_activity_pre.valid_activity.len()
            );
        }
    }
}
