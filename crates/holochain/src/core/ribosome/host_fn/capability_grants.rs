use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::CapabilityGrantsInput;
use holochain_zome_types::CapabilityGrantsOutput;
use std::sync::Arc;

/// list all the grants stored locally in the chain filtered by tag
/// this is only the current grants as per local CRUD
pub fn capability_grants(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: CapabilityGrantsInput,
) -> RibosomeResult<CapabilityGrantsOutput> {
    unimplemented!();
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::conductor::dna_store::MockDnaStore;
    use crate::core::ribosome::ZomeCall;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::test_utils::test_conductor::TestZomeCall;
    use crate::{conductor::ConductorBuilder, test_utils::test_conductor::TestConductorHandle};
    use ::fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_state::test_utils::test_environments;
    use holochain_types::cell::CellId;
    use holochain_types::dna::zome::Zome;
    use holochain_types::dna::DnaFile;
    use holochain_types::fixt::CapSecretFixturator;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_capability_secret_test<'a>() {
        holochain_types::observability::test_run().ok();
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        let _output: CapSecret =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "cap_secret", ());
    }

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_transferable_cap_grant<'a>() {
        holochain_types::observability::test_run().ok();
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        let secret: CapSecret =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "cap_secret", ());
        let header: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Capability,
            "transferable_cap_grant",
            secret
        );
        let entry: GetOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "get_entry", header);

        let entry_secret: CapSecret = match entry.into_inner() {
            Some(element) => {
                let cap_grant_entry: CapGrantEntry = element.entry().to_grant_option().unwrap();
                match cap_grant_entry.access {
                    CapAccess::Transferable { secret, .. } => secret,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };
        assert_eq!(entry_secret, secret,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_authorized_call() {
        let (dna_file, _) = DnaFile::unique_from_test_wasms(vec![TestWasm::Capability])
            .await
            .unwrap();

        let alice_agent_id = fake_agent_pubkey_1();
        let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());

        let bob_agent_id = fake_agent_pubkey_2();
        let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());

        let mut dna_store = MockDnaStore::new();

        dna_store.expect_get().return_const(Some(dna_file.clone()));
        dna_store.expect_add_dna().return_const(());
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store
            .expect_add_entry_defs::<Vec<_>>()
            .times(2)
            .return_const(());

        let envs = test_environments();
        let handle: TestConductorHandle = ConductorBuilder::with_mock_dna_store(dna_store)
            .test(&envs)
            .await
            .unwrap()
            .into();

        let _ = handle
            .setup_app_for_all_agents_with_no_membrane_proof(
                "app-",
                &[alice_agent_id.clone(), bob_agent_id.clone()],
                &[dna_file],
            )
            .await;

        let zome: &Zome = &TestWasm::Capability.into();

        // ALICE FAILING AN UNAUTHED CALL

        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
        pub struct CapFor(CapSecret, AgentPubKey);

        let original_secret = CapSecretFixturator::new(Unpredictable).next().unwrap();

        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::Capability.into(),
                cap: None,
                fn_name: "try_cap_claim".into(),
                payload: ExternInput::new(
                    CapFor(original_secret, bob_agent_id.clone().try_into().unwrap())
                        .try_into()
                        .unwrap(),
                ),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        // the _outer_ invocation response is to try_cap_claim for alice
        // the _inner_ invocation response is needs_cap_claim and should be unauthorized
        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let response: SerializedBytes = guest_output.into_inner();
                let inner_response: ZomeCallResponse = response.try_into().unwrap();
                // the inner response should be unauthorized
                assert_matches!(inner_response, ZomeCallResponse::Unauthorized(_, _, _, _));
            }
            _ => unreachable!(),
        }

        // BOB COMMITS A TRANSFERABLE GRANT WITH THE SECRET SHARED WITH ALICE

        let original_grant_hash: HeaderHash = handle
            .call_zome_ok_flat(
                &bob_cell_id,
                zome.zome_name().clone(),
                "transferable_cap_grant",
                None,
                None,
                original_secret,
            )
            .await;

        // ALICE CAN NOW CALL THE AUTHED REMOTE FN

        let response: ZomeCallResponse = handle
            .call_zome_ok_struct(TestZomeCall {
                cell_id: &alice_cell_id,
                zome,
                cap: None,
                fn_name: "try_cap_claim",
                payload: CapFor(original_secret, bob_agent_id.clone()),
                provenance: None,
            })
            .await;

        assert_eq!(
            response,
            ZomeCallResponse::Ok(ExternOutput::new(().try_into().unwrap())),
        );

        // BOB ROLLS THE GRANT SO ONLY THE NEW ONE WILL WORK FOR ALICE

        let new_grant_header_hash: HeaderHash = handle
            .call_zome_ok_struct(TestZomeCall {
                cell_id: &bob_cell_id.clone(),
                zome,
                cap: None,
                fn_name: "roll_cap_grant",
                payload: ExternInput::new(original_grant_hash.try_into().unwrap()),
                provenance: None,
            })
            .await;

        let output = handle
            .call_zome(ZomeCall {
                cell_id: bob_cell_id.clone(),
                zome_name: TestWasm::Capability.into(),
                cap: None,
                fn_name: "get_entry".into(),
                payload: ExternInput::new(new_grant_header_hash.clone().try_into().unwrap()),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        let new_secret: CapSecret = match output.clone() {
            ZomeCallResponse::Ok(guest_output) => {
                let get_output: GetOutput = guest_output.into_inner().try_into().unwrap();
                match get_output.into_inner() {
                    Some(element) => match element.entry().to_grant_option() {
                        Some(zome_call_cap_grant) => match zome_call_cap_grant.access {
                            CapAccess::Transferable { secret, .. } => secret,
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    },
                    _ => unreachable!("Couldn't get {:?}", new_grant_header_hash),
                }
            }
            _ => unreachable!(),
        };

        let output: ZomeCallResponse = handle
            .call_zome_ok_struct(TestZomeCall {
                cell_id: &alice_cell_id,
                zome,
                cap: None,
                fn_name: "try_cap_claim",
                payload: CapFor(original_secret, bob_agent_id.clone().try_into().unwrap()),
                provenance: None,
            })
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output: ZomeCallResponse = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::Capability.into(),
                cap: None,
                fn_name: "try_cap_claim".into(),
                payload: ExternInput::new(
                    CapFor(new_secret, bob_agent_id.clone().try_into().unwrap())
                        .try_into()
                        .unwrap(),
                ),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            output,
            ZomeCallResponse::Ok(ExternOutput::new(().try_into().unwrap())),
        );

        // BOB DELETES THE GRANT SO NO SECRETS WORK

        let _output = handle
            .call_zome(ZomeCall {
                cell_id: bob_cell_id,
                zome_name: TestWasm::Capability.into(),
                cap: None,
                fn_name: "delete_cap_grant".into(),
                payload: ExternInput::new(new_grant_header_hash.try_into().unwrap()),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        let output: ZomeCallResponse = handle
            .call_zome_ok_struct(TestZomeCall {
                cell_id: &alice_cell_id,
                zome,
                cap: None,
                fn_name: "try_cap_claim",
                payload: CapFor(original_secret, bob_agent_id.clone().try_into().unwrap()),
                provenance: None,
            })
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output = handle
            .call_zome(ZomeCall {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::Capability.into(),
                cap: None,
                fn_name: "try_cap_claim".into(),
                payload: ExternInput::new(
                    CapFor(new_secret, bob_agent_id.clone().try_into().unwrap())
                        .try_into()
                        .unwrap(),
                ),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        // the _outer_ invocation response is to try_cap_claim for alice
        // the _inner_ invocation response is needs_cap_claim and should be unauthorized
        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let response: SerializedBytes = guest_output.into_inner();
                let inner_response: ZomeCallResponse = response.try_into().unwrap();
                // the inner response should be unauthorized
                assert_matches!(inner_response, ZomeCallResponse::Unauthorized(_, _, _, _));
            }
            _ => unreachable!(),
        }

        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap();
    }
}
