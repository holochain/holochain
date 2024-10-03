use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use holochain::prelude::*;
use holochain::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn call_to_init_passes() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let is_init_called = Arc::new(AtomicBool::new(false));
    let is_init_called_clone = is_init_called.clone();
    let zome = SweetInlineZomes::new(vec![], 0)
        .function("init", move |_, _: ()| {
            is_init_called_clone.store(true, Ordering::Relaxed);
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
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

    assert!(is_init_called.load(Ordering::Relaxed));
}

#[tokio::test(flavor = "multi_thread")]
async fn call_init_from_init_across_cells() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let inits = Arc::new(AtomicU8::new(0));
    let inits1 = inits.clone();
    let inits2 = inits.clone();
    let zome1 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    access: ().into(),
                    functions: GrantedFunctions::Listed(
                        vec![("no-init".into(), "xxx".into())].into_iter().collect(),
                    ),
                }),
                ChainTopOrdering::default(),
            ))?;
            inits1.fetch_add(1, Ordering::SeqCst);
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_api, _: ()| {
            // just triggers init
            Ok(())
        });
    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome1).await;
    let cellid1 = CellId::new(dna1.dna_hash().clone(), agent.clone());

    let zome2 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            api.call(vec![Call {
                target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cellid1.clone())),
                zome_name: SweetInlineZomes::COORDINATOR.into(),
                fn_name: "touch".into(),
                cap_secret: None,
                payload: ExternIO::encode(()).unwrap(),
            }])?;
            inits2.fetch_add(1, Ordering::SeqCst);
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_api, _: ()| {
            // just triggers init
            Ok(())
        });
    let (dna2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna1, dna2])
        .await
        .unwrap();
    let (_cell1, cell2) = app.into_tuple();

    let () = conductor
        .call(&cell2.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await;

    assert_eq!(inits.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = "error: Serialize(Deserialize(\"invalid value: integer `42`")]
async fn call_init_with_invalid_return_type() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome = SweetInlineZomes::new(vec![], 0)
        .function("init", |_, _: ()| Ok(42))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
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
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = "error: Serialize(Deserialize(\\\"invalid value: integer `42`")]
async fn call_init_with_invalid_return_type_across_cells() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome_1 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |_, _: ()| Ok(42))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
            Ok(())
        });
    let (dna_1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_1).await;
    let cell_id_1 = CellId::new(dna_1.dna_hash().clone(), agent.clone());

    let zome_2 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            api.call(vec![Call {
                target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cell_id_1.clone())),
                zome_name: SweetInlineZomes::COORDINATOR.into(),
                fn_name: "touch".into(),
                cap_secret: None,
                payload: ExternIO::encode(()).unwrap(),
            }])?;
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
            Ok(())
        });
    let (dna_2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_1, dna_2])
        .await
        .unwrap();
    let (_cell_1, cell_2) = app.into_tuple();

    let () = conductor
        .call(&cell_2.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(
    expected = "SerializationError(Deserialize(\"invalid type: unit value, expected usize\"))"
)]
async fn call_init_with_invalid_parameters() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome = SweetInlineZomes::new(vec![], 0)
        .function("init", |_, _: usize| Ok(InitCallbackResult::Pass))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
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
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(
    expected = "error: Host(\"Deserialize(\\\"invalid type: unit value, expected usize\\\")\")"
)]
async fn call_init_with_invalid_parameters_across_cells() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let zome_1 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |_, _: usize| Ok(InitCallbackResult::Pass))
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
            Ok(())
        });
    let (dna_1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_1).await;
    let cell_id_1 = CellId::new(dna_1.dna_hash().clone(), agent.clone());

    let zome_2 = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            api.call(vec![Call {
                target: CallTarget::ConductorCell(CallTargetCell::OtherCell(cell_id_1.clone())),
                zome_name: SweetInlineZomes::COORDINATOR.into(),
                fn_name: "touch".into(),
                cap_secret: None,
                payload: ExternIO::encode(()).unwrap(),
            }])?;
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_, _: ()| {
            // Simple Zome to just trigger a call to init.
            Ok(())
        });
    let (dna_2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome_2).await;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_1, dna_2])
        .await
        .unwrap();
    let (_cell_1, cell_2) = app.into_tuple();

    let () = conductor
        .call(&cell_2.zome(SweetInlineZomes::COORDINATOR), "touch", ())
        .await;
}
