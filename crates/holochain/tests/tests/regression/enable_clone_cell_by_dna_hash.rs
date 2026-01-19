use holochain::sweettest::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

/// Regression test for https://github.com/holochain/holochain/issues/4572
///
/// When calling EnableCloneCell with a DnaHash (instead of CloneId) on an already-enabled clone,
/// the operation should succeed as a no-op, just as it does when called with a CloneId.
///
/// Previously, get_disabled_clone_id only searched disabled_clones when given a DnaHash,
/// causing a CloneCellNotFound error when the clone was already enabled.
#[tokio::test(flavor = "multi_thread")]
async fn enable_clone_cell_by_dna_hash_on_active_clone() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    // Create a clone cell
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone1".to_string()),
        membrane_proof: None,
        name: Some("clone1".to_string()),
    };
    let cloned_cell: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let clone_dna_hash = cloned_cell.cell_id.dna_hash().clone();

    // Now try to enable the already-enabled clone using its DnaHash (the bug scenario)
    // This should succeed as a no-op, not return CloneCellNotFound
    let request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::DnaHash(clone_dna_hash.clone()),
    };
    let _: ClonedCell = conductor.call(&zome, "enable_clone", request).await;

    // Verify using CloneId also works on enabled clone (this already worked before the fix)
    let request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: ClonedCell = conductor.call(&zome, "enable_clone", request).await;

    // Now test the same with a disabled clone
    let request = DisableCloneCellInput {
        clone_cell_id: CloneCellId::DnaHash(clone_dna_hash.clone()),
    };
    let _: () = conductor.call(&zome, "disable_clone", request).await;

    // Re-enable using DnaHash should work
    let request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::DnaHash(clone_dna_hash.clone()),
    };
    let _: ClonedCell = conductor.call(&zome, "enable_clone", request).await;
}
