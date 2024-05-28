use holo_hash::{ActionHash, AgentPubKey};
use holochain_conductor_services::{DpkiServiceError, KeyState};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::action::ActionType;
use holochain_zome_types::record::Record;
use holochain_zome_types::timestamp::Timestamp;
use matches::assert_matches;

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::{conductor::ConductorError, CellError};
use crate::core::workflow::WorkflowError;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::{
    SweetConductor, SweetConductorConfig, SweetDnaFile, SweetInlineZomes, SweetZome,
};

#[tokio::test(flavor = "multi_thread")]
async fn delete_agent_key() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, coordinator_zomes) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd]).await;
    let app = conductor
        .setup_app("", [&("role".to_string(), dna_file.clone())])
        .await
        .unwrap();
    let agent_key = app.agent().clone();
    let zome = SweetZome::new(
        app.cells()[0].cell_id().clone(),
        coordinator_zomes[0].name.clone(),
    );
    let create_fn_name = "create";
    let read_fn_name = "reed";

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
        .delete_agent_key_for_app(non_existing_key.clone(), app.installed_app_id().clone())
        .await;
    assert_matches!(
        result,
        Err(ConductorError::DpkiError(DpkiServiceError::DpkiAgentMissing(key))) if key == non_existing_key
    );

    // writing to the cell should succeed
    let action_hash: ActionHash = conductor.call(&zome, create_fn_name, ()).await;

    // TODOs
    // - add multiple cells
    // - disable cell cloning
    // - make chains read-only

    // deleting the key should succeed
    let result = conductor
        .clone()
        .delete_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
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
        .delete_agent_key_for_app(agent_key.clone(), app.installed_app_id().clone())
        .await;
    assert_matches!(result, Err(ConductorError::DpkiError(DpkiServiceError::DpkiAgentInvalid(invalid_key, _))) if invalid_key == agent_key);

    // reading an entry should still succeed
    let result: Option<Record> = conductor.call(&zome, read_fn_name, action_hash).await;
    assert!(result.is_some());

    // creating an entry should fail now
    let result = conductor
        .call_fallible::<_, ActionHash>(&zome, create_fn_name, ())
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = result {
        assert_matches!(
            *workflow_error,
            WorkflowError::SysValidationError(SysValidationError::ValidationOutcome(ValidationOutcome::DpkiAgentInvalid(invalid_key, _))) if invalid_key == agent_key
        );
    } else {
        panic!("different error than expected");
    }

    // last source chain action should be CloseChain
    conductor
        .get_or_create_authored_db(dna_file.dna_hash(), agent_key)
        .unwrap()
        .test_read(|txn| {
            txn.query_row(
                "SELECT type FROM Action ORDER BY seq DESC LIMIT 1",
                [],
                |row| {
                    let action_type = row.get::<_, String>("type").unwrap();
                    assert_eq!(action_type, ActionType::CloseChain.to_string());
                    Ok(())
                },
            )
            .unwrap();
        });
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
        .delete_agent_key_for_app(agent_key, app.installed_app_id().clone())
        .await;
    assert_matches!(
        result,
        Err(ConductorError::DpkiError(
            DpkiServiceError::DpkiNotInstalled
        ))
    );
}
