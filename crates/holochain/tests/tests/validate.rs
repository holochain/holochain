use assert2::{assert, let_assert};
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::CellError;
use holochain::core::workflow::WorkflowError;
use holochain::prelude::*;
use holochain::sweettest::*;
use holochain_state::source_chain::SourceChainError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn call_to_validate_in_inline_zomes_passes() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let is_validate_called = Arc::new(AtomicBool::new(false));
    let is_validate_called_clone = is_validate_called.clone();
    let zome = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", move |_, _: Op| {
            is_validate_called_clone.store(true, Ordering::Relaxed);
            Ok(ValidateCallbackResult::Valid)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let app = conductor
        .setup_app_for_agent("app", agent, &[dna])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    let () = conductor
        .call(&cell.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await;

    assert!(is_validate_called.load(Ordering::Relaxed));
}

#[tokio::test(flavor = "multi_thread")]
async fn call_validate_across_cells_passes() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let is_validate_called = Arc::new(AtomicBool::new(false));
    let is_validate_called_clone = is_validate_called.clone();
    let zome_1 = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", move |_, _: Op| {
            is_validate_called_clone.store(true, Ordering::Relaxed);
            Ok(ValidateCallbackResult::Valid)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });
    let (dna_1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_1).await;
    let cell_id_1 = CellId::new(dna_1.dna_hash().clone(), agent.clone());

    let zome_2 = SweetInlineZomes::new(vec![], 0).function("cross_cell_call", move |api, _: ()| {
        // Simple Zome to just call the other zome.
        api.call(vec![Call {
            target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cell_id_1.clone())),
            zome_name: SweetInlineZomes::COORDINATOR.into(),
            fn_name: "touch".into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
        }])?;

        Ok(())
    });
    let (dna_2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_1, dna_2])
        .await
        .unwrap();
    let (_cell_1, cell_2) = app.into_tuple();

    let () = conductor
        .call(
            &cell_2.zome(SweetInlineZomes::COORDINATOR),
            "cross_cell_call",
            (),
        )
        .await;

    assert!(is_validate_called.load(Ordering::Relaxed));
}

#[tokio::test(flavor = "multi_thread")]
async fn call_validate_with_invalid_return_type_in_inline_zomes() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", |_, _: Op| Ok(42))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let app = conductor
        .setup_app_for_agent("app", agent, &[dna])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    let err = conductor
        .call_fallible::<_, ()>(&cell.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await
        .unwrap_err();

    let_assert!(ConductorApiError::CellError(CellError::WorkflowError(workflow_err)) = err);
    let_assert!(
        WorkflowError::SourceChainError(SourceChainError::Other(other_err)) = *workflow_err
    );
    // Can't downcast the `Box<dyn Error>` to a concrete type so just compare the error message.
    assert!(other_err
        .to_string()
        .starts_with("The callback has an invalid return type: invalid value: integer `42`"));
}

#[tokio::test(flavor = "multi_thread")]
async fn call_validate_with_invalid_return_type_across_cells() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome_1 = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", |_, _: Op| Ok(42))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });
    let (dna_1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_1).await;
    let cell_id_1 = CellId::new(dna_1.dna_hash().clone(), agent.clone());

    let zome_2 = SweetInlineZomes::new(vec![], 0).function("cross_cell_call", move |api, _: ()| {
        // Simple Zome to just trigger a call to validate.
        api.call(vec![Call {
            target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cell_id_1.clone())),
            zome_name: SweetInlineZomes::COORDINATOR.into(),
            fn_name: "touch".into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
        }])?;

        Ok(())
    });
    let (dna_2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_1, dna_2])
        .await
        .unwrap();
    let (_cell_1, cell_2) = app.into_tuple();

    let err = conductor
        .call_fallible::<_, ()>(
            &cell_2.zome(SweetInlineZomes::COORDINATOR),
            "cross_cell_call",
            (),
        )
        .await
        .unwrap_err();

    let_assert!(ConductorApiError::CellError(other_err) = err);
    // Can't downcast the `Box<dyn Error>` to a concrete type so just compare the error message.
    assert!(other_err
        .to_string()
        .contains("The callback has an invalid return type: invalid value: integer `42`"));
}

#[tokio::test(flavor = "multi_thread")]
async fn call_validate_with_invalid_parameters_in_inline_zomes() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", |_, _: usize| Ok(ValidateCallbackResult::Valid))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let app = conductor
        .setup_app_for_agent("app", agent, &[dna])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    let err = conductor
        .call_fallible::<_, ()>(&cell.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await
        .unwrap_err();

    let_assert!(ConductorApiError::CellError(CellError::WorkflowError(workflow_err)) = err);
    let_assert!(
        WorkflowError::SourceChainError(SourceChainError::Other(other_err)) = *workflow_err
    );
    // Can't downcast the `Box<dyn Error>` to a concrete type so just compare the error message.
    assert!(
        other_err.to_string()
            == "The callback has invalid parameters: wrong msgpack marker FixMap(1)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn call_validate_with_invalid_parameters_across_cells() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome_1 = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", move |_, _: usize| {
            Ok(ValidateCallbackResult::Valid)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to validate.
            Ok(())
        });
    let (dna_1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_1).await;
    let cell_id_1 = CellId::new(dna_1.dna_hash().clone(), agent.clone());

    let zome_2 = SweetInlineZomes::new(vec![], 0).function("cross_cell_call", move |api, _: ()| {
        // Simple Zome to call the other zome
        api.call(vec![Call {
            target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cell_id_1.clone())),
            zome_name: SweetInlineZomes::COORDINATOR.into(),
            fn_name: "touch".into(),
            cap_secret: None,
            payload: ExternIO::encode(()).unwrap(),
        }])?;

        Ok(())
    });
    let (dna_2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_1, dna_2])
        .await
        .unwrap();
    let (_cell_1, cell_2) = app.into_tuple();

    let err = conductor
        .call_fallible::<_, ()>(
            &cell_2.zome(SweetInlineZomes::COORDINATOR),
            "cross_cell_call",
            (),
        )
        .await
        .unwrap_err();

    let_assert!(ConductorApiError::CellError(other_err) = err);
    // Can't downcast the `Box<dyn Error>` to a concrete type so just compare the error message.
    assert!(other_err
        .to_string()
        .contains("The callback has invalid parameters: wrong msgpack marker FixMap(1)"));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_an_update_returns_both_updates() {
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    
    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct TestEntry {
        value: String,
    }
    
    let entry_def = EntryDef::default_from_id("test_entry");
    let zome = SweetInlineZomes::new(vec![entry_def], 0)
        .function("create_and_update_twice", |api, _: ()| {
            // Create an entry
            let entry = TestEntry {
                value: "original".to_string(),
            };
            let create_hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::app(entry.try_into().unwrap()).unwrap(),
                ChainTopOrdering::default(),
            ))?;
            
            // Update it once
            let entry_v2 = TestEntry {
                value: "updated_once".to_string(),
            };
            let update_hash = api.update(UpdateInput::new(
                create_hash.clone(),
                Entry::app(entry_v2.try_into().unwrap()).unwrap(),
                ChainTopOrdering::default(),
            ))?;
            
            // Update the update
            let entry_v3 = TestEntry {
                value: "updated_twice".to_string(),
            };
            let _update_update_hash = api.update(UpdateInput::new(
                update_hash,
                Entry::app(entry_v3.try_into().unwrap()).unwrap(),
                ChainTopOrdering::default(),
            ))?;
            
            Ok(create_hash)
        })
        .function("get_details", |api, hash: ActionHash| {
            let details = api.get_details(vec![GetInput::new(
                hash.into(),
                GetOptions::local(),
            )])?;
            Ok(details)
        });
    
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let app = conductor
        .setup_app_for_agent("app", agent, &[dna])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);
    
    // Call the function that creates and updates twice
    let create_hash: ActionHash = conductor
        .call(&zome, "create_and_update_twice", ())
        .await;
    
    // Get details of the original create action
    let details: Vec<Option<Details>> = conductor
        .call(&zome, "get_details", create_hash)
        .await;
    
    // Verify we get details
    assert_eq!(details.len(), 1);
    let details = details[0].as_ref().expect("Expected details to be present");
    
    // Extract updates from the RecordDetails
    let record_details = match details {
        Details::Record(record_details) => record_details,
        _ => panic!("Expected RecordDetails"),
    };
    
    // Assert that we have two updates
    assert_eq!(
        record_details.updates.len(),
        2,
        "Expected two updates (one direct update and one update of the update), but got {}",
        record_details.updates.len()
    );
}
