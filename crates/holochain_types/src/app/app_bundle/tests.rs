use super::AppBundle;
use crate::prelude::*;
use app_manifest_v0::tests::{app_manifest_fixture, app_manifest_properties_fixture};

async fn app_bundle_fixture(modifiers: DnaModifiersOpt<YamlProperties>) -> (AppBundle, DnaFile) {
    let dna_wasm = DnaWasmHashed::from_content(DnaWasm::new_invalid()).await;
    let fake_wasms = vec![dna_wasm.clone().into_content()];
    let fake_zomes = vec![IntegrityZome::new(
        "hi".into(),
        ZomeDef::Wasm(WasmZome::new(dna_wasm.as_hash().clone(), None)).into(),
    )];
    let dna_def_1 = DnaDef::unique_from_zomes(fake_zomes.clone(), vec![]);

    let dna1 = DnaFile::new(dna_def_1, fake_wasms.clone()).await;

    let manifest = app_manifest_fixture(
        Some("path1".to_string()),
        DnaHash::with_data_sync(dna1.dna_def()),
        modifiers,
    )
    .await;

    let resources = vec![(
        "path1".to_string(),
        DnaBundle::from_dna_file(dna1.clone()).unwrap(),
    )];

    let bundle = AppBundle::new(manifest.into(), resources).unwrap();
    (bundle, dna1)
}

/// Test that an app with a single Created cell can be provisioned
#[tokio::test]
async fn provisioning_1_create() {
    holochain_trace::test_run();

    let modifiers = DnaModifiersOpt {
        properties: Some(app_manifest_properties_fixture()),
        network_seed: Some("network_seed".into()),
    };
    let (bundle, dna) = app_bundle_fixture(modifiers).await;

    // Apply the modifier overrides specified in the manifest fixture
    let dna = dna
        .with_network_seed("network_seed".to_string())
        .await
        .with_properties(SerializedBytes::try_from(app_manifest_properties_fixture()).unwrap())
        .await;

    let resolution = bundle
        .resolve_cells(
            &std::collections::HashMap::new(),
            Default::default(),
            Default::default(),
        )
        .await
        .unwrap();

    // Build the expected output.
    // NB: this relies heavily on the particulars of the `app_manifest_fixture`
    let role = AppRolePrimary::new(dna.dna_hash().to_owned(), true, 50).into();

    let expected = AppRoleResolution {
        dnas_to_register: vec![(dna, None)],
        role_assignments: vec![("role_name".into(), role)],
    };
    assert_eq!(resolution, expected);
}
