use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_conductor_services::{DpkiServiceError, KeyState};
use holochain_state::source_chain::SourceChainError;
use holochain_types::app::{AppError, CreateCloneCellPayload};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::action::ActionType;
use holochain_zome_types::dependencies::holochain_integrity_types::DnaModifiersOpt;
use holochain_zome_types::record::Record;
use holochain_zome_types::timestamp::Timestamp;
use matches::assert_matches;
use rusqlite::Row;

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::{conductor::ConductorError, CellError};
use crate::core::workflow::WorkflowError;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::{SweetConductor, SweetConductorConfig, SweetDnaFile, SweetZome};

#[tokio::test(flavor = "multi_thread")]
async fn revoke_agent_key_with_dpki_installed() {
    holochain_trace::test_run();
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file_1, _, coordinator_zomes_1) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_file_2, _, coordinator_zomes_2) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_1 = "role_1";
    let role_2 = "role_2";
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
    let create_fn_name = "create_entry";
    let read_fn_name = "get_post";

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
    let action_hash_1: ActionHash = conductor.call(&zome_1, create_fn_name, ()).await;
    let action_hash_2: ActionHash = conductor.call(&zome_2, create_fn_name, ()).await;

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
        .get_or_create_authored_db(dna_file_1.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read({
            let row_fn = row_fn.clone();
            move |txn| txn.query_row(sql, [], row_fn).unwrap()
        });
    conductor
        .get_or_create_authored_db(dna_file_2.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read(move |txn| txn.query_row(sql, [], row_fn).unwrap());

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
    let result: Option<Record> = conductor.call(&zome_1, read_fn_name, action_hash_1).await;
    assert!(result.is_some());
    let result: Option<Record> = conductor.call(&zome_2, read_fn_name, action_hash_2).await;
    assert!(result.is_some());

    // Creating an entry should fail now for both cells
    let result = conductor
        .call_fallible::<_, ActionHash>(&zome_1, create_fn_name, ())
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = result {
        assert_matches!(
            *workflow_error,
            WorkflowError::SysValidationError(SysValidationError::ValidationOutcome(ValidationOutcome::DpkiAgentInvalid(invalid_key, _))) if invalid_key == agent_key
        );
    } else {
        panic!("different error than expected");
    }
    let result = conductor
        .call_fallible::<_, ActionHash>(&zome_2, create_fn_name, ())
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = result {
        assert_matches!(
            *workflow_error,
            WorkflowError::SysValidationError(SysValidationError::ValidationOutcome(ValidationOutcome::DpkiAgentInvalid(invalid_key, _))) if invalid_key == agent_key
        );
    } else {
        panic!("different error than expected");
    }

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
    let no_dpki_conductor_config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(no_dpki_conductor_config).await;
    let (dna_file_1, _, coordinator_zomes_1) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_file_2, _, coordinator_zomes_2) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_1 = "role_1";
    let role_2 = "role_2";
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
    let create_fn_name = "create_entry";
    let read_fn_name = "get_post";

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
    let action_hash_1: ActionHash = conductor.call(&zome_1, create_fn_name, ()).await;
    let action_hash_2: ActionHash = conductor.call(&zome_2, create_fn_name, ()).await;

    // Deleting the key should succeed
    let revocation_result_per_cell = conductor
        .clone()
        .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
        .await
        .unwrap();
    assert_matches!(revocation_result_per_cell.get(&cell_id_1).unwrap(), Ok(()));
    assert_matches!(revocation_result_per_cell.get(&cell_id_2).unwrap(), Ok(()));

    // Last source chain action in both cells should be 'Delete' action of the agent key
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
        .get_or_create_authored_db(dna_file_1.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read({
            let row_fn = row_fn.clone();
            move |txn| txn.query_row(sql, [], row_fn).unwrap()
        });
    conductor
        .get_or_create_authored_db(dna_file_2.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read(move |txn| txn.query_row(sql, [], row_fn).unwrap());

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
    let result: Option<Record> = conductor.call(&zome_1, read_fn_name, action_hash_1).await;
    assert!(result.is_some());
    let result: Option<Record> = conductor.call(&zome_2, read_fn_name, action_hash_2).await;
    assert!(result.is_some());

    // Creating an entry should fail now for both cells
    let result = conductor
        .call_fallible::<_, ActionHash>(&zome_1, create_fn_name, ())
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = result {
        assert_matches!(
            *workflow_error,
            WorkflowError::SourceChainError(
                SourceChainError::InvalidCommit(message)
            ) if message == ValidationOutcome::InvalidAgentKey(agent_key.clone()).to_string()
        );
    } else {
        panic!("different error than expected {result:?}");
    }
    let result = conductor
        .call_fallible::<_, ActionHash>(&zome_2, create_fn_name, ())
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = result {
        assert_matches!(
            *workflow_error,
            WorkflowError::SourceChainError(
                SourceChainError::InvalidCommit(message)
            ) if message == ValidationOutcome::InvalidAgentKey(agent_key.clone()).to_string()
        );
    } else {
        panic!("different error than expected {result:?}");
    }

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
