use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductor, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;

// Intended to keep https://github.com/holochain/holochain/issues/2868 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_entry_types_does_not_prevent_deletes() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateRejectAppTypes, TestWasm::Crd])
            .await;

    let (cell,) = conductor
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let created: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "create",
            (),
        )
        .await;

    let _: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "delete_via_hash",
            created,
        )
        .await;
}

// Intended to keep https://github.com/holochain/holochain/issues/2868 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_link_types_does_not_prevent_delete_links() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::ValidateRejectAppTypes,
        TestWasm::Link,
    ])
    .await;

    let (cell,) = conductor
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let created: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Link.coordinator_zome_name()),
            "create_link",
            (),
        )
        .await;

    let _: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Link.coordinator_zome_name()),
            "delete_link",
            created,
        )
        .await;
}

pub mod must_get_agent_activity_saturation;
