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
use matches::assert_matches;
use rusqlite::Row;

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::{conductor::ConductorError, CellError};
use crate::core::workflow::WorkflowError;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::{
    await_consistency, SweetConductor, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
    SweetZome,
};

use super::SweetApp;

#[tokio::test(flavor = "multi_thread")]
async fn revoke_agent_key_with_dpki_installed() {
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
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);
    create_clone_cell_payload.role_name = role_2.to_string();
    let result = conductor
        .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
        .await
        .unwrap_err();
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);
}

#[tokio::test(flavor = "multi_thread")]
async fn revoke_agent_key_without_dpki_installed() {
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
async fn retry_revoke_agent_key_to_recover_from_partial_revocation_with_dpki() {
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

    // Revoke agent key in Deepkey
    revoke_agent_key_in_dpki(&conductor, agent_key.clone()).await;

    // Verify key is invalid
    {
        let dpki_service = conductor
            .running_services()
            .dpki
            .expect("dpki must be installed");
        let dpki_state = dpki_service.state().await;
        let timestamp = Timestamp::now();
        let key_state = dpki_state
            .key_state(agent_key.clone(), timestamp)
            .await
            .unwrap();
        assert_matches!(key_state, KeyState::Invalid(_));
    }

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

    // Creating an entry should fail now for both cells as the key is invalid in Deepkey.
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
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);
    create_clone_cell_payload.role_name = role_2.to_string();
    let result = conductor
        .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
        .await
        .unwrap_err();
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);

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
async fn retry_revoke_agent_key_to_recover_from_partial_revocation_without_dpki() {
    let TestCase {
        conductor,
        app,
        agent_key,
        cell_id_1,
        cell_id_2,
        ..
    } = TestCase::no_dpki().await;

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
async fn delete_agent_key_is_received_by_other_conductor() {
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
            SweetConductorConfig::standard()
        } else {
            SweetConductorConfig::standard().no_dpki()
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
