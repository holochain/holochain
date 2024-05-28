use holo_hash::{ActionHash, AgentPubKey};
use holochain_conductor_services::{DpkiServiceError, KeyState};
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
use crate::sweettest::{
    SweetConductor, SweetConductorConfig, SweetDnaFile, SweetInlineZomes, SweetZome,
};

#[tokio::test(flavor = "multi_thread")]
async fn revoke_agent_key() {
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
    let zome_1 = SweetZome::new(
        app.cells()[0].cell_id().clone(),
        coordinator_zomes_1[0].name.clone(),
    );
    let zome_2 = SweetZome::new(
        app.cells()[1].cell_id().clone(),
        coordinator_zomes_2[0].name.clone(),
    );
    let create_fn_name = "create_entry";
    let read_fn_name = "get_post";

    // no agent key provided, so DPKI should be installed
    // and the generated agent key be valid
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

    // deleting a non-existing key should fail
    let non_existing_key = AgentPubKey::from_raw_32(vec![0; 32]);
    let result = conductor
        .clone()
        .revoke_agent_key_for_app(non_existing_key.clone(), app.installed_app_id().clone())
        .await;
    assert_matches!(
        result,
        Err(ConductorError::DpkiError(DpkiServiceError::DpkiAgentMissing(key))) if key == non_existing_key
    );

    // writing to the cell should succeed
    let action_hash_1: ActionHash = conductor.call(&zome_1, create_fn_name, ()).await;
    let action_hash_2: ActionHash = conductor.call(&zome_2, create_fn_name, ()).await;

    // deleting the key should succeed
    let result = conductor
        .clone()
        .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
        .await;
    assert_matches!(result, Ok((_, _)));

    // key should be invalid
    let key_state = dpki
        .state()
        .await
        .key_state(agent_key.clone(), Timestamp::now())
        .await
        .unwrap();
    assert_matches!(key_state, KeyState::Invalid(_));

    // deleting agent key again should return a "key invalid" error
    let result = conductor
        .clone()
        .revoke_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
        .await;
    assert_matches!(result, Err(ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _))) if invalid_key == agent_key);

    // reading an entry should still succeed
    let result: Option<Record> = conductor.call(&zome_1, read_fn_name, action_hash_1).await;
    assert!(result.is_some());
    let result: Option<Record> = conductor.call(&zome_2, read_fn_name, action_hash_2).await;
    assert!(result.is_some());

    // creating an entry should fail now for both cells
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

    // last source chain action in both cells should be CloseChain
    let sql = "SELECT type FROM Action ORDER BY seq DESC LIMIT 1";
    let row_fn = |row: &Row| {
        let action_type = row.get::<_, String>("type").unwrap();
        assert_eq!(action_type, ActionType::CloseChain.to_string());
        Ok(())
    };
    conductor
        .get_or_create_authored_db(dna_file_1.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read(move |txn| {
            txn.query_row(sql, [], row_fn.clone()).unwrap();
        });
    conductor
        .get_or_create_authored_db(dna_file_2.dna_hash(), agent_key.clone())
        .unwrap()
        .test_read(move |txn| {
            txn.query_row(sql, [], row_fn).unwrap();
        });

    // cloning cells should fail for both cells
    let mut create_clone_cell_payload = CreateCloneCellPayload {
        role_name: role_1.to_string(),
        membrane_proof: None,
        modifiers: DnaModifiersOpt::none().with_network_seed("network_seed".into()),
        name: None,
    };
    let result = conductor
        .raw_handle()
        .create_clone_cell(app.installed_app_id(), create_clone_cell_payload.clone())
        .await
        .unwrap_err();
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);
    create_clone_cell_payload.role_name = role_2.to_string();
    let result = conductor
        .raw_handle()
        .create_clone_cell(app.installed_app_id(), create_clone_cell_payload)
        .await
        .unwrap_err();
    assert_matches!(result, ConductorError::AppError(AppError::CellToCloneHasInvalidAgent(invalid_key)) if invalid_key == agent_key);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_agent_key_without_dpki_installed_fails() {
    // spawn a conductor without dpki installed
    let conductor_config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(conductor_config).await;
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let app = conductor
        .setup_app("", [&("role".to_string(), dna_file)])
        .await
        .unwrap();
    let agent_key = app.agent().clone();

    // calling delete key without dpki installed should return an error
    let result = conductor
        .clone()
        .revoke_agent_key_for_app(agent_key, app.installed_app_id().clone())
        .await;
    assert_matches!(
        result,
        Err(ConductorError::DpkiError(
            DpkiServiceError::DpkiNotInstalled
        ))
    );
}
