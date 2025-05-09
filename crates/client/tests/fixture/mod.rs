use bytes::Bytes;
use holochain::prelude::DnaModifiersOpt;
use holochain_types::app::{
    AppManifest, AppManifestV1, AppRoleDnaManifest, AppRoleManifest, CellProvisioning,
};
use holochain_types::dna::{
    CoordinatorManifest, DnaBundle, DnaManifest, DnaManifestV1, IntegrityManifest,
    ValidatedDnaManifest, ZomeDependency, ZomeManifest,
};
use holochain_types::prelude::AppBundle;
use holochain_wasm_test_utils::{TestWasm, TestWasmPair};
use mr_bundle::ResourceBytes;
use std::path::PathBuf;
use std::sync::OnceLock;

pub fn get_fixture_app_bundle() -> Bytes {
    static TEST_APP_BUNDLE: OnceLock<Bytes> = OnceLock::new();

    TEST_APP_BUNDLE.get_or_init(make_fixture_app_bundle).clone()
}

fn make_fixture_app_bundle() -> Bytes {
    let dna_manifest = ValidatedDnaManifest::try_from(DnaManifest::V1(DnaManifestV1 {
        name: "test-dna".to_string(),
        coordinator: CoordinatorManifest {
            zomes: vec![ZomeManifest {
                name: "foo".into(),
                hash: None,
                dependencies: Some(vec![ZomeDependency {
                    name: "foo_integrity".into(),
                }]),
                path: "test_wasm_client.wasm".to_string(),
            }],
        },
        integrity: IntegrityManifest {
            network_seed: None,
            properties: None,
            zomes: vec![ZomeManifest {
                name: "foo_integrity".into(),
                hash: None,
                dependencies: None,
                path: "integrity_test_wasm_client.wasm".to_string(),
            }],
        },
        #[cfg(feature = "unstable-migration")]
        lineage: vec![],
    }))
    .unwrap();

    let (integrity, coordinator) = get_test_wasm_pair(TestWasm::Client);
    let dna_bundle = DnaBundle::new(
        dna_manifest,
        vec![
            ("integrity_test_wasm_client.wasm".to_string(), integrity),
            ("test_wasm_client.wasm".to_string(), coordinator),
        ],
    )
    .unwrap();

    let app_manifest = AppManifest::V1(AppManifestV1 {
        name: "fixture".to_string(),
        description: None,
        allow_deferred_memproofs: false,
        roles: vec![AppRoleManifest {
            name: "foo".to_string(),
            provisioning: Some(CellProvisioning::Create { deferred: false }),
            dna: AppRoleDnaManifest {
                path: Some("test.dna".to_string()),
                modifiers: DnaModifiersOpt::none(),
                installed_hash: None,
                clone_limit: 10,
            },
        }],
    });

    let app = AppBundle::new(app_manifest, vec![("test.dna".to_string(), dna_bundle)]).unwrap();

    app.pack().unwrap()
}

fn get_test_wasm_pair(wasm: TestWasm) -> (ResourceBytes, ResourceBytes) {
    let base = PathBuf::from("../test_utils/wasm/wasm_workspace/target/");
    let TestWasmPair {
        integrity: integrity_path,
        coordinator: coordinator_path,
    } = TestWasmPair::<PathBuf>::from(wasm);

    let integrity = std::fs::read(base.join(integrity_path)).unwrap();
    let coordinator = std::fs::read(base.join(coordinator_path)).unwrap();

    (integrity.into(), coordinator.into())
}
