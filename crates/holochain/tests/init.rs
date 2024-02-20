use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use holochain::prelude::*;
use holochain::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn call_init_from_init_across_cells() {
    let mut conductor = SweetConductor::from_standard_config().await;
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
