use std::path::PathBuf;

use crate::prelude::*;
use ::fixt::prelude::*;
use app_manifest_v1::tests::{app_manifest_fixture, app_manifest_properties_fixture};

use super::AppBundle;

async fn app_bundle_fixture() -> (AppBundle, DnaFile) {
    let dna_wasm = DnaWasmHashed::from_content(DnaWasm::new_invalid()).await;
    let fake_wasms = vec![dna_wasm.clone().into_content()];
    let fake_zomes = vec![IntegrityZome::new(
        "hi".into(),
        ZomeDef::Wasm(WasmZome::new(dna_wasm.as_hash().clone())).into(),
    )];
    let dna_def_1 = DnaDef::unique_from_zomes(fake_zomes.clone(), vec![]);
    let dna_def_2 = DnaDef::unique_from_zomes(fake_zomes, vec![]);

    let dna1 = DnaFile::new(dna_def_1, fake_wasms.clone()).await;
    let dna2 = DnaFile::new(dna_def_2, fake_wasms.clone()).await;

    let path1 = PathBuf::from(format!("{}", dna1.dna_hash()));

    let (manifest, _dna_hashes) = app_manifest_fixture(
        Some(DnaLocation::Bundled(path1.clone())),
        vec![dna1.dna_def().clone(), dna2.dna_def().clone()],
    )
    .await;

    let resources = vec![(path1, DnaBundle::from_dna_file(dna1.clone()).await.unwrap())];

    let bundle = AppBundle::new(manifest, resources, PathBuf::from("."))
        .await
        .unwrap();
    (bundle, dna1)
}

/// Test that an app with a single Created cell can be provisioned
#[tokio::test]
async fn provisioning_1_create() {
    observability::test_run().ok();
    let agent = fixt!(AgentPubKey);
    let (bundle, dna) = app_bundle_fixture().await;

    // Apply the modifier overrides specified in the manifest fixture
    let dna = dna
        .with_network_seed("network_seed".to_string())
        .await
        .with_properties(SerializedBytes::try_from(app_manifest_properties_fixture()).unwrap())
        .await;

    let cell_id = CellId::new(dna.dna_hash().to_owned(), agent.clone());

    let resolution = bundle
        .resolve_cells(agent.clone(), DnaGamut::placeholder(), Default::default())
        .await
        .unwrap();

    // Build the expected output.
    // NB: this relies heavily on the particulars of the `app_manifest_fixture`
    let role = AppRoleAssignment::new(cell_id, true, 50);

    let expected = AppRoleResolution {
        agent,
        dnas_to_register: vec![(dna, None)],
        role_assignments: vec![("role_id".into(), role)],
    };
    assert_eq!(resolution, expected);
}
