use holo_hash::{ActionHash, AgentPubKey, DnaHash, EntryHash};
use holochain_conductor_services::{DpkiServiceError, KeyRevocation, KeyState, RevokeKeyInput};
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_state::source_chain::{SourceChain, SourceChainError};
use holochain_types::app::{AppError, CreateCloneCellPayload};
use holochain_types::deepkey_roundtrip_backward;
use holochain_types::dna::DnaFile;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::action::ActionType;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::dependencies::holochain_integrity_types::{DnaModifiersOpt, Signature};
use holochain_zome_types::record::Record;
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::validate::ValidationStatus;
use matches::assert_matches;
use rusqlite::{named_params, Row};

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::{conductor::ConductorError, CellError};
use crate::core::workflow::WorkflowError;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::{
    await_consistency, SweetConductor, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
    SweetZome,
};

use super::SweetApp;

mod single_conductor {

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn revoke_agent_key_with_dpki() {
        holochain_trace::test_run();
        let TestCase {
            mut conductor,
            dna_file_1,
            dna_file_2,
            role_1,
            role_2,
            app,
            agent_key,
            cell_id_1,
            cell_id_2,
            zome_1,
            zome_2,
            create_fn_name,
            read_fn_name,
        } = TestCase::dpki().await;

        // No agent key provided, so the installed DPKI service will be used to generate an agent key
        let dpki = conductor
            .running_services()
            .dpki
            .expect("dpki must be running");
        let key_state = dpki
            .state()
            .await
            .key_state(agent_key.clone(), Timestamp::now())
            .await
            .unwrap();
        assert_matches!(key_state, KeyState::Valid(_));

        // Deleting a non-existing key should fail
        let non_existing_key = AgentPubKey::from_raw_32(vec![0; 32]);
        let result = conductor
            .clone()
            .revoke_agent_key_for_app(non_existing_key.clone(), app.installed_app_id().clone())
            .await;
        assert_matches!(
            result,
            Err(ConductorError::DpkiError(DpkiServiceError::DpkiAgentMissing(key))) if key == non_existing_key
        );

        // Writing to cells should succeed
        let action_hash_1: ActionHash = conductor.call(&zome_1, &*create_fn_name, ()).await;
        let action_hash_2: ActionHash = conductor.call(&zome_2, &*create_fn_name, ()).await;

        // Deleting the key should succeed
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Ok(()));
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Ok(()));

        // Key should be in invalid in DPKI
        let key_state = dpki
            .state()
            .await
            .key_state(agent_key.clone(), Timestamp::now())
            .await
            .unwrap();
        assert_matches!(key_state, KeyState::Invalid(_));

        // Last source chain action in both cells should be 'Delete' action of the agent key
        assert_delete_agent_key_present_in_source_chain(
            agent_key.clone(),
            &conductor,
            dna_file_1.dna_hash(),
        );
        assert_delete_agent_key_present_in_source_chain(
            agent_key.clone(),
            &conductor,
            dna_file_2.dna_hash(),
        );

        // Deleting same agent key again should succeed, even though the key is not deleted again.
        // The call itself is successful and should contain errors for the individual cells.
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_1);
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_2);

        // Reading an entry should still succeed
        let result: Option<Record> = conductor.call(&zome_1, &*read_fn_name, action_hash_1).await;
        assert!(result.is_some());
        let result: Option<Record> = conductor.call(&zome_2, &*read_fn_name, action_hash_2).await;
        assert!(result.is_some());

        // Creating an entry should fail now for both cells
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_1, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_dpki_agent_key(error, agent_key.clone());
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_2, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_dpki_agent_key(error, agent_key.clone());

        // Cloning cells should fail for both cells
        let mut create_clone_cell_payload = CreateCloneCellPayload {
            role_name: role_1.to_string(),
            membrane_proof: None,
            modifiers: DnaModifiersOpt::none().with_network_seed("network_seed".into()),
            name: None,
        };
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload.clone())
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _timestamp)) if invalid_key == agent_key);
        create_clone_cell_payload.role_name = role_2.to_string();
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _timestamp)) if invalid_key == agent_key);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn revoke_agent_key_without_dpki() {
        let TestCase {
            mut conductor,
            dna_file_1,
            dna_file_2,
            role_1,
            role_2,
            app,
            agent_key,
            cell_id_1,
            cell_id_2,
            zome_1,
            zome_2,
            create_fn_name,
            read_fn_name,
        } = TestCase::no_dpki().await;

        // Deleting a non-existing key should fail
        let non_existing_key = AgentPubKey::from_raw_32(vec![0; 32]);
        let result = conductor
            .clone()
            .revoke_agent_key_for_app(non_existing_key.clone(), app.installed_app_id().clone())
            .await;
        assert_matches!(
            result,
            Err(ConductorError::AppError(AppError::AgentKeyMissing(key, app_id))) if key == non_existing_key && app_id == *app.installed_app_id()
        );

        // Writing to cells should succeed
        let action_hash_1: ActionHash = conductor.call(&zome_1, &*create_fn_name, ()).await;
        let action_hash_2: ActionHash = conductor.call(&zome_2, &*create_fn_name, ()).await;

        // Deleting the key should succeed
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Ok(()));
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Ok(()));

        // Last source chain action in both cells should be 'Delete' action of the agent key
        assert_delete_agent_key_present_in_source_chain(
            agent_key.clone(),
            &conductor,
            dna_file_1.dna_hash(),
        );
        assert_delete_agent_key_present_in_source_chain(
            agent_key.clone(),
            &conductor,
            dna_file_2.dna_hash(),
        );

        // Deleting same agent key again should succeed, even though the key is not deleted again.
        // The call itself is successful and should contain errors for the individual cells.
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_1);
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_2);

        // Reading an entry should still succeed
        let result: Option<Record> = conductor.call(&zome_1, &*read_fn_name, action_hash_1).await;
        assert!(result.is_some());
        let result: Option<Record> = conductor.call(&zome_2, &*read_fn_name, action_hash_2).await;
        assert!(result.is_some());

        // Creating an entry should fail now for both cells
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_1, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_agent_key_in_source_chain(error, agent_key.clone());
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_2, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_agent_key_in_source_chain(error, agent_key.clone());

        // Cloning cells should fail for both cells
        let mut create_clone_cell_payload = CreateCloneCellPayload {
            role_name: role_1.to_string(),
            membrane_proof: None,
            modifiers: DnaModifiersOpt::none().with_network_seed("network_seed".into()),
            name: None,
        };
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload.clone())
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::SourceChainError(SourceChainError::InvalidAgentKey(invalid_key, cell_id)) if invalid_key == agent_key && cell_id == cell_id_1);
        create_clone_cell_payload.role_name = role_2.to_string();
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::SourceChainError(SourceChainError::InvalidAgentKey(invalid_key, cell_id)) if invalid_key == agent_key && cell_id == cell_id_2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn recover_from_partial_revocation_with_dpki() {
        let TestCase {
            mut conductor,
            role_1,
            role_2,
            app,
            agent_key,
            cell_id_1,
            cell_id_2,
            zome_1,
            zome_2,
            create_fn_name,
            read_fn_name,
            ..
        } = TestCase::dpki().await;

        // Writing to cells should succeed
        let action_hash_1: ActionHash = conductor.call(&zome_1, &*create_fn_name, ()).await;
        let action_hash_2: ActionHash = conductor.call(&zome_2, &*create_fn_name, ()).await;

        // Revoke agent key in Dpki
        revoke_agent_key_in_dpki(&conductor, agent_key.clone()).await;

        // Delete agent key of cell 1 of the app
        let source_chain_1 = SourceChain::new(
            conductor
                .get_or_create_authored_db(cell_id_1.dna_hash(), agent_key.clone())
                .unwrap(),
            conductor.get_dht_db(cell_id_1.dna_hash()).unwrap(),
            conductor.get_dht_db_cache(cell_id_1.dna_hash()).unwrap(),
            conductor.keystore().clone(),
            agent_key.clone(),
        )
        .await
        .unwrap();
        source_chain_1.delete_valid_agent_pub_key().await.unwrap();
        source_chain_1
            .flush(
                &conductor
                    .holochain_p2p()
                    .to_dna(cell_id_1.dna_hash().clone(), conductor.get_chc(&cell_id_1)),
            )
            .await
            .unwrap();

        // Check agent key is invalid in cell 1
        let invalid_agent_key_error = source_chain_1
            .valid_create_agent_key_action()
            .await
            .unwrap_err();
        assert_matches!(invalid_agent_key_error, SourceChainError::InvalidAgentKey(invalid_key, cell_id) if invalid_key == agent_key && cell_id == cell_id_1);

        // Reading an entry should still succeed
        let result: Option<Record> = conductor.call(&zome_1, &*read_fn_name, action_hash_1).await;
        assert!(result.is_some());
        let result: Option<Record> = conductor.call(&zome_2, &*read_fn_name, action_hash_2).await;
        assert!(result.is_some());

        // Creating an entry should fail now for both cells as the key is invalid in Dpki.
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_1, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_dpki_agent_key(error, agent_key.clone());
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_2, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_dpki_agent_key(error, agent_key.clone());

        // Cloning cells should fail for both cells
        let mut create_clone_cell_payload = CreateCloneCellPayload {
            role_name: role_1.to_string(),
            membrane_proof: None,
            modifiers: DnaModifiersOpt::none().with_network_seed("network_seed".into()),
            name: None,
        };
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload.clone())
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _timestamp)) if invalid_key == agent_key);
        create_clone_cell_payload.role_name = role_2.to_string();
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _timestamp)) if invalid_key == agent_key);

        // Calling key revocation should succeed and return an error result for cell 1
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_1);
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Ok(()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn recover_from_partial_revocation_without_dpki() {
        let TestCase {
            mut conductor,
            role_1,
            role_2,
            app,
            agent_key,
            cell_id_1,
            cell_id_2,
            zome_1,
            zome_2,
            create_fn_name,
            read_fn_name,
            ..
        } = TestCase::no_dpki().await;

        // Writing to cells should succeed
        let action_hash_1: ActionHash = conductor.call(&zome_1, &*create_fn_name, ()).await;
        let action_hash_2: ActionHash = conductor.call(&zome_2, &*create_fn_name, ()).await;

        // Delete agent key of cell 1 of the app
        let source_chain_1 = SourceChain::new(
            conductor
                .get_or_create_authored_db(cell_id_1.dna_hash(), agent_key.clone())
                .unwrap(),
            conductor.get_dht_db(cell_id_1.dna_hash()).unwrap(),
            conductor.get_dht_db_cache(cell_id_1.dna_hash()).unwrap(),
            conductor.keystore().clone(),
            agent_key.clone(),
        )
        .await
        .unwrap();
        source_chain_1.delete_valid_agent_pub_key().await.unwrap();
        source_chain_1
            .flush(
                &conductor
                    .holochain_p2p()
                    .to_dna(cell_id_1.dna_hash().clone(), conductor.get_chc(&cell_id_1)),
            )
            .await
            .unwrap();

        // Check agent key is invalid in cell 1
        let invalid_agent_key_error = source_chain_1
            .valid_create_agent_key_action()
            .await
            .unwrap_err();
        assert_matches!(invalid_agent_key_error, SourceChainError::InvalidAgentKey(invalid_key, cell_id) if invalid_key == agent_key && cell_id == cell_id_1);

        // Reading an entry should still succeed
        let result: Option<Record> = conductor.call(&zome_1, &*read_fn_name, action_hash_1).await;
        assert!(result.is_some());
        let result: Option<Record> = conductor.call(&zome_2, &*read_fn_name, action_hash_2).await;
        assert!(result.is_some());

        // Creating an entry should fail for cell 1 as the agent key is invalid.
        let error = conductor
            .call_fallible::<_, ActionHash>(&zome_1, &*create_fn_name, ())
            .await
            .unwrap_err();
        assert_error_due_to_invalid_agent_key_in_source_chain(error, agent_key.clone());
        // Creating an entry should succeed for cell 2 as the agent key is still valid.
        let _ = conductor
            .call_fallible::<_, ActionHash>(&zome_2, &*create_fn_name, ())
            .await
            .unwrap();

        // Cloning cells should fail for cell 1 as the agent key is invalid.
        let mut create_clone_cell_payload = CreateCloneCellPayload {
            role_name: role_1.to_string(),
            membrane_proof: None,
            modifiers: DnaModifiersOpt::none().with_network_seed("network_seed".into()),
            name: None,
        };
        let result = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload.clone())
            .await
            .unwrap_err();
        assert_matches!(result, ConductorError::SourceChainError(SourceChainError::InvalidAgentKey(invalid_key, cell_id)) if invalid_key == agent_key && cell_id == cell_id_1);
        // Cloning cells should succeed for cell 2 as the agent key is still valid.
        create_clone_cell_payload.role_name = role_2.to_string();
        let _ = conductor
            .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
            .await
            .unwrap();

        // Calling key revocation should succeed and return an error result for cell 1
        let revocation_result_per_cell = conductor
            .clone()
            .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(key, cell_id))) if *key == agent_key && *cell_id == cell_id_1);
        assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Ok(()));
    }
}

mod multi_conductor {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn revoke_agent_key_without_dpki() {
        holochain_trace::test_run();
        let no_dpki_conductor_config = SweetConductorConfig::rendezvous(true).no_dpki();
        let mut conductors =
            SweetConductorBatch::from_config_rendezvous(2, no_dpki_conductor_config).await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let role = "role";
        let apps = conductors
            .setup_app("", [&(role.to_string(), dna_file.clone())])
            .await
            .unwrap();
        let cells = apps.cells_flattened();
        let alice = cells[0].agent_pubkey().clone();

        await_consistency(20, &cells).await.unwrap();

        // Deleting the key should succeed
        let revocation_result_per_cell = conductors[0]
            .clone()
            .revoke_agent_key_for_app(alice.clone(), apps[0].installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(
            revocation_result_per_cell.get(cells[0].cell_id()).unwrap(),
            Ok(())
        );

        await_consistency(20, &cells).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn revoke_agent_key_with_dpki() {
        holochain_trace::test_run();
        let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let role = "role";
        let mut apps = conductors
            .setup_app("", [&(role.to_string(), dna_file.clone())])
            .await
            .unwrap()
            .into_inner()
            .into_iter();
        let alice_app = apps.next().unwrap();
        let bob_app = apps.next().unwrap();
        let alice = alice_app.agent().clone();
        let bob = bob_app.agent().clone();
        let alice_cell = alice_app.into_cells().into_iter().next().unwrap();
        let bob_cell = bob_app.into_cells().into_iter().next().unwrap();

        // Await Dpki consistency of Alice's and Bob's conductors.
        await_consistency(
            60,
            [
                &conductors[0].dpki_cell().unwrap(),
                &conductors[1].dpki_cell().unwrap(),
            ],
        )
        .await
        .unwrap();

        // Await app cell consistency
        await_consistency(60, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Alice's key should be valid on Alice's conductor.
        assert_key_valid_in_dpki(&conductors[0], alice.clone()).await;
        // Bob's key should be valid on Alice's conductor.
        assert_key_valid_in_dpki(&conductors[0], bob.clone()).await;
        // Alice's key should be valid on Bob's conductor.
        assert_key_valid_in_dpki(&conductors[1], alice.clone()).await;
        // Bob's key should be valid on Bob's conductor.
        assert_key_valid_in_dpki(&conductors[1], bob.clone()).await;

        // Revoke Alice's key
        {
            let dpki = conductors[0].running_services().dpki.unwrap();
            let dpki_state = dpki.state().await;
            let key_meta = dpki_state.query_key_meta(alice.clone()).await.unwrap();
            // Sign revocation request
            let signature = dpki
                .cell_id
                .agent_pubkey()
                .sign_raw(
                    &conductors[0].keystore,
                    key_meta.key_registration_addr.get_raw_39().into(),
                )
                .await
                .unwrap();
            let signature = deepkey_roundtrip_backward!(Signature, &signature);
            // Revoke key in DPKI
            let _revocation = dpki_state
                .revoke_key(RevokeKeyInput {
                    key_revocation: KeyRevocation {
                        prior_key_registration: key_meta.key_registration_addr,
                        revocation_authorization: vec![(0, signature)],
                    },
                })
                .await
                .unwrap();

            // Alice's key should be invalid on Alice's conductor.
            let key_state = dpki_state
                .key_state(alice.clone(), Timestamp::now())
                .await
                .unwrap();
            assert_matches!(key_state, KeyState::Invalid(_));
        }

        // Await Dpki consistency of Alice's and Bob's conductors.
        await_consistency(
            30,
            [
                &conductors[0].dpki_cell().unwrap(),
                &conductors[1].dpki_cell().unwrap(),
            ],
        )
        .await
        .unwrap();

        // Alice's key should be invalid on Bob's conductor.
        {
            let dpki = conductors[1].running_services().dpki.unwrap();
            let dpki_state = dpki.state().await;
            let key_state = dpki_state
                .key_state(alice.clone(), Timestamp::now())
                .await
                .unwrap();
            assert_matches!(key_state, KeyState::Invalid(_));
        }

        // Delete Alice's key on the source chain
        let mut alice_source_chain = conductors[0]
            .get_agent_source_chain(&alice, dna_file.dna_hash())
            .await;
        delete_agent_key_from_source_chain(
            &conductors[0],
            &mut alice_source_chain,
            alice_cell.cell_id(),
        )
        .await;

        // Await app cell consistency.
        await_consistency(60, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Check Alice's agent key `Delete` has been accepted by Alice's validation.
        assert_delete_agent_key_accepted_by_validation(&alice, &conductors[0], dna_file.dna_hash());
        // Check Alice's agent key `Delete` has been accepted by Bob's validation.
        assert_delete_agent_key_accepted_by_validation(&alice, &conductors[1], dna_file.dna_hash());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn recover_from_partial_revocation_with_dpki() {
        holochain_trace::test_run();
        let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
        let (dna_file_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let (dna_file_2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let role_1 = "role_1".to_string();
        let role_2 = "role_2".to_string();
        let mut apps = conductors
            .setup_app(
                "",
                [
                    &(role_1.to_string(), dna_file_1.clone()),
                    &(role_2.to_string(), dna_file_2.clone()),
                ],
            )
            .await
            .unwrap()
            .into_iter();
        let alice_app = apps.next().unwrap();
        let alice = alice_app.agent().clone();
        let alice_cell_1 = alice_app.cells().first().unwrap();
        let alice_cell_2 = alice_app.cells().get(1).unwrap();
        let bob_app = apps.next().unwrap();
        let bob_cell_1 = bob_app.cells().first().unwrap();
        let bob_cell_2 = bob_app.cells().get(1).unwrap();

        // Await initial DHT sync of Dpki and both cells
        await_consistency(
            20,
            [
                &conductors[0].dpki_cell().unwrap(),
                &conductors[1].dpki_cell().unwrap(),
            ],
        )
        .await
        .unwrap();
        await_consistency(20, [alice_cell_1, bob_cell_1])
            .await
            .unwrap();
        await_consistency(5, [alice_cell_2, bob_cell_2])
            .await
            .unwrap();

        // Revoke agent key in Dpki
        revoke_agent_key_in_dpki(&conductors[0], alice.clone()).await;

        // Await for revocation to reach bob's Dpki
        await_consistency(
            20,
            [
                &conductors[0].dpki_cell().unwrap(),
                &conductors[1].dpki_cell().unwrap(),
            ],
        )
        .await
        .unwrap();

        // Delete agent key of cell 1 of the app and publish and integrate ops
        let mut alice_source_chain_1 = conductors[0]
            .get_agent_source_chain(&alice, dna_file_1.dna_hash())
            .await;
        delete_agent_key_from_source_chain(
            &conductors[0],
            &mut alice_source_chain_1,
            alice_cell_1.cell_id(),
        )
        .await;

        // Check agent key is invalid in cell 1
        let invalid_agent_key_error = alice_source_chain_1
            .valid_create_agent_key_action()
            .await
            .unwrap_err();
        assert_matches!(invalid_agent_key_error, SourceChainError::InvalidAgentKey(invalid_key, cell_id) if invalid_key == alice && cell_id == *alice_cell_1.cell_id());

        // Wait for key deletion on source chain to sync with bob
        await_consistency(20, [alice_cell_1, bob_cell_1])
            .await
            .unwrap();

        // Check Alice's agent key `Delete` in cell 1 has been accepted by Alice's validation.
        assert_delete_agent_key_accepted_by_validation(
            &alice,
            &conductors[0],
            dna_file_1.dna_hash(),
        );
        // Check Alice's agent key `Delete` in cell 1 has been accepted by Bob's validation.
        assert_delete_agent_key_accepted_by_validation(
            &alice,
            &conductors[1],
            dna_file_1.dna_hash(),
        );

        // Calling key revocation should succeed and return an error result for cell 1
        let revocation_result_per_cell = conductors[0]
            .clone()
            .revoke_agent_key_for_app(alice.clone(), alice_app.installed_app_id().clone())
            .await
            .unwrap();
        assert_matches!(revocation_result_per_cell.get(alice_cell_1.cell_id()).unwrap(), Err(ConductorApiError::SourceChainError(SourceChainError::InvalidAgentKey(invalid_key, cell_id))) if *invalid_key == alice && *cell_id == *alice_cell_1.cell_id());

        // Await consistency of cell 2
        await_consistency(20, [alice_cell_2, bob_cell_2])
            .await
            .unwrap();

        // Check Alice's agent key `Delete` in cell 2 has been accepted by Alice's validation.
        assert_delete_agent_key_accepted_by_validation(
            &alice,
            &conductors[0],
            dna_file_2.dna_hash(),
        );
        // Check Alice's agent key `Delete` in cell 2 has been accepted by Bob's validation.
        assert_delete_agent_key_accepted_by_validation(
            &alice,
            &conductors[1],
            dna_file_2.dna_hash(),
        );
    }
}

struct TestCase {
    conductor: SweetConductor,
    dna_file_1: DnaFile,
    dna_file_2: DnaFile,
    role_1: String,
    role_2: String,
    app: SweetApp,
    agent_key: AgentPubKey,
    cell_id_1: CellId,
    cell_id_2: CellId,
    zome_1: SweetZome,
    zome_2: SweetZome,
    create_fn_name: String,
    read_fn_name: String,
}

impl TestCase {
    async fn new(dpki: bool) -> TestCase {
        let conductor_config = if dpki {
            SweetConductorConfig::rendezvous(false)
                .apply_shared_rendezvous()
                .await
        } else {
            SweetConductorConfig::rendezvous(false)
                .apply_shared_rendezvous()
                .await
                .no_dpki()
        };
        let mut conductor = SweetConductor::from_config(conductor_config).await;
        let (dna_file_1, _, coordinator_zomes_1) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let (dna_file_2, _, coordinator_zomes_2) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let role_1 = "role_1".to_string();
        let role_2 = "role_2".to_string();
        let app = conductor
            .setup_app(
                "",
                [
                    &(role_1.to_string(), dna_file_1.clone()),
                    &(role_2.to_string(), dna_file_2.clone()),
                ],
            )
            .await
            .unwrap();
        let agent_key = app.agent().clone();
        let cell_id_1 = app.cells()[0].cell_id().clone();
        let cell_id_2 = app.cells()[1].cell_id().clone();
        let zome_1 = SweetZome::new(cell_id_1.clone(), coordinator_zomes_1[0].name.clone());
        let zome_2 = SweetZome::new(cell_id_2.clone(), coordinator_zomes_2[0].name.clone());
        let create_fn_name = "create_entry".to_string();
        let read_fn_name = "get_post".to_string();
        TestCase {
            conductor,
            dna_file_1,
            dna_file_2,
            role_1,
            role_2,
            app,
            agent_key,
            cell_id_1,
            cell_id_2,
            zome_1,
            zome_2,
            create_fn_name,
            read_fn_name,
        }
    }

    async fn dpki() -> TestCase {
        TestCase::new(true).await
    }

    async fn no_dpki() -> TestCase {
        TestCase::new(false).await
    }
}

fn assert_delete_agent_key_present_in_source_chain(
    agent_key: AgentPubKey,
    conductor: &SweetConductor,
    dna_hash: &DnaHash,
) {
    let sql = "\
        SELECT author, type, deletes_entry_hash
        FROM Action
        ORDER BY seq DESC";
    let row_fn = {
        let agent_key = agent_key.clone();
        move |row: &Row| {
            let author = row.get::<_, AgentPubKey>("author").unwrap();
            let action_type = row.get::<_, String>("type").unwrap();
            let deletes_entry_hash = row.get::<_, EntryHash>("deletes_entry_hash").unwrap();
            assert_eq!(author, agent_key);
            assert_eq!(action_type, ActionType::Delete.to_string());
            assert_eq!(deletes_entry_hash, agent_key.clone().into());
            Ok(())
        }
    };
    conductor
        .get_or_create_authored_db(dna_hash, agent_key.clone())
        .unwrap()
        .test_read(move |txn| txn.query_row(sql, [], row_fn).unwrap());
}

fn assert_delete_agent_key_accepted_by_validation(
    agent_key: &AgentPubKey,
    conductor: &SweetConductor,
    dna_hash: &DnaHash,
) {
    let sql = "\
        SELECT Action.author, Action.type, Action.deletes_entry_hash
        FROM Action
        JOIN DhtOp On DhtOp.action_hash = Action.hash
        WHERE DhtOp.validation_status = :valid_status
        AND Action.deletes_entry_hash IS NOT NULL
        ORDER BY seq DESC";
    conductor.get_dht_db(dna_hash).unwrap().test_read({
        let agent_key = agent_key.clone();
        move |txn| {
            let mut stmt = txn.prepare(sql).unwrap();
            let rows: Vec<_> = stmt
                .query_map(
                    named_params! { ":valid_status": ValidationStatus::Valid },
                    |row| {
                        let author = row.get::<_, AgentPubKey>("author").unwrap();
                        let action_type = row.get::<_, String>("type").unwrap();
                        let deletes_entry_hash =
                            row.get::<_, EntryHash>("deletes_entry_hash").unwrap();
                        assert_eq!(author, agent_key.clone());
                        assert_eq!(action_type, ActionType::Delete.to_string());
                        assert_eq!(deletes_entry_hash, agent_key.clone().into());
                        Ok(())
                    },
                )
                .unwrap()
                .collect();
            assert!(!rows.is_empty());
        }
    });
}

async fn assert_key_valid_in_dpki(conductor: &SweetConductor, agent_key: AgentPubKey) {
    let dpki = conductor.running_services().dpki.unwrap();
    let dpki_state = dpki.state().await;
    let key_state = dpki_state
        .key_state(agent_key, Timestamp::now())
        .await
        .unwrap();
    assert_matches!(key_state, KeyState::Valid(_));
}

fn assert_error_due_to_invalid_dpki_agent_key(error: ConductorApiError, agent_key: AgentPubKey) {
    if let ConductorApiError::CellError(CellError::WorkflowError(workflow_error)) = error {
        assert_matches!(
            *workflow_error,
            WorkflowError::SysValidationError(SysValidationError::ValidationOutcome(ValidationOutcome::DpkiAgentInvalid(invalid_key, _timestamp))) if invalid_key == agent_key.clone()
        );
    } else {
        panic!("different error than expected {error}");
    }
}

fn assert_error_due_to_invalid_agent_key_in_source_chain(
    error: ConductorApiError,
    agent_key: AgentPubKey,
) {
    if let ConductorApiError::CellError(CellError::WorkflowError(workflow_error)) = error {
        assert_matches!(
            *workflow_error,
            WorkflowError::SourceChainError(
                SourceChainError::InvalidCommit(message)
            ) if message == ValidationOutcome::InvalidAgentKey(agent_key.clone()).to_string()
        );
    } else {
        panic!("different error than expected {error}");
    }
}

async fn revoke_agent_key_in_dpki(conductor: &SweetConductor, agent_key: AgentPubKey) {
    let dpki_service = conductor
        .running_services()
        .dpki
        .expect("dpki must be installed");
    let dpki_state = dpki_service.state().await;
    let timestamp = Timestamp::now();
    match dpki_state
        .key_state(agent_key.clone(), timestamp)
        .await
        .unwrap()
    {
        KeyState::Valid(_) => {
            // Get action hash of key registration
            let key_meta = dpki_state.query_key_meta(agent_key.clone()).await.unwrap();
            // Sign revocation request
            let signature = dpki_service
                .cell_id
                .agent_pubkey()
                .sign_raw(
                    &conductor.keystore(),
                    key_meta.key_registration_addr.get_raw_39().into(),
                )
                .await
                .unwrap();
            let signature = deepkey_roundtrip_backward!(Signature, &signature);
            // Revoke key in DPKI
            let _revocation = dpki_state
                .revoke_key(RevokeKeyInput {
                    key_revocation: KeyRevocation {
                        prior_key_registration: key_meta.key_registration_addr,
                        revocation_authorization: vec![(0, signature)],
                    },
                })
                .await
                .unwrap();
        }
        _state => panic!("key must be valid but is {_state:?}"),
    }
}

async fn delete_agent_key_from_source_chain(
    conductor: &SweetConductor,
    source_chain: &mut SourceChain,
    cell_id: &CellId,
) {
    source_chain.delete_valid_agent_pub_key().await.unwrap();
    source_chain
        .flush(
            &conductor
                .holochain_p2p()
                .to_dna(cell_id.dna_hash().clone(), conductor.get_chc(cell_id)),
        )
        .await
        .unwrap();
    conductor
        .get_cell_triggers(cell_id)
        .await
        .unwrap()
        .publish_dht_ops
        .trigger(&"key deletion");
    conductor
        .get_cell_triggers(cell_id)
        .await
        .unwrap()
        .integrate_dht_ops
        .trigger(&"key deletion");
}
