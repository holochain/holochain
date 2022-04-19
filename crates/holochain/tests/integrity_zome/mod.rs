use holo_hash::HeaderHash;
use holochain::sweettest::*;
use holochain_types::prelude::DnaWasm;
use holochain_wasm_test_utils::TestCoordinatorWasm;
use holochain_wasm_test_utils::TestIntegrityWasm;
use holochain_zome_types::CoordinatorZome;
use holochain_zome_types::Element;

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_hot_swap() {
    let mut conductor = SweetConductor::from_config(Default::default()).await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await
    .unwrap();
    let dna_hash = dna.dna_hash().clone();

    println!("Install Dna with integrity and coordinator zomes.");
    let app = conductor.setup_app("app", &[dna]).await.unwrap();
    let cells = app.into_cells();

    println!("Create entry from the coordinator zome into the integrity zome.");
    let hash: HeaderHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;
    println!("Success!");

    println!("Try getting the entry from the coordinator zome.");
    let element: Option<Element> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(element.is_some());
    println!("Success!");

    println!("Hot swap the coordinator zomes for a totally different coordinator zome (conductor is still running)");
    conductor
        .hot_swap_coordinators(
            &dna_hash,
            vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZomeUpdate).into_inner()],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();
    println!("Success!");

    println!("Try getting the entry from the new coordinator zome.");
    let element: Option<Element> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(element.is_some());
    println!("Success! Success! Success! ");
}
