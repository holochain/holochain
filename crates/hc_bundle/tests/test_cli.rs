use assert_cmd::prelude::*;
use holochain_types::web_app::WebAppManifest;
use holochain_types::{prelude::*, web_app::WebAppBundle};
use holochain_util::ffs;
use jsonschema::JSONSchema;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::Duration,
};
use walkdir::WalkDir;

fn read_app(path: &Path) -> anyhow::Result<AppBundle> {
    Ok(AppBundle::decode(&ffs::sync::read(path).unwrap())?)
}

fn read_dna(path: &Path) -> anyhow::Result<DnaBundle> {
    Ok(DnaBundle::decode(&ffs::sync::read(path).unwrap())?)
}

fn read_web_app(path: &Path) -> anyhow::Result<WebAppBundle> {
    Ok(WebAppBundle::decode(&ffs::sync::read(path).unwrap())?)
}

#[tokio::test]
async fn roundtrip() {
    {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/dnas/dna1"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/dnas/dna2"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-app").unwrap();
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-web-app").unwrap();
        let cmd = cmd.args(["pack", "tests/fixtures/web-app/"]);
        cmd.assert().success();
    }

    let web_app_path = PathBuf::from("tests/fixtures/web-app/fixture-web-app.webhapp");
    let app_path = PathBuf::from("tests/fixtures/my-app/fixture-app.happ");
    let dna1_path = PathBuf::from("tests/fixtures/my-app/dnas/dna1/a dna.dna");
    let dna2_path = PathBuf::from("tests/fixtures/my-app/dnas/dna2/another dna.dna");

    let _original_web_happ = read_web_app(&web_app_path).unwrap();
    let _original_happ = read_app(&app_path).unwrap();
    let _original_dna1 = read_dna(&dna1_path).unwrap();
    let _original_dna2 = read_dna(&dna2_path).unwrap();
}

#[tokio::test]
async fn test_packed_hash_consistency() {
    let mut i = 0;
    let mut hash = None;
    while i < 5 {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/dnas/dna1"]);
        cmd.assert().success();

        let cmd = Command::new("sha256sum")
            .args([r"./tests/fixtures/my-app/dnas/dna1/a dna.dna"])
            .unwrap();
        let sha_result = std::str::from_utf8(&cmd.stdout).unwrap().to_string();
        let sha_result = sha_result.split(' ').collect::<Vec<_>>();
        let new_hash = sha_result.first().unwrap().to_owned().to_owned();

        match hash {
            Some(prev_hash) => {
                assert_eq!(prev_hash, new_hash);
                hash = Some(new_hash)
            }
            None => hash = Some(new_hash),
        }
        i += 1;
    }
}

#[tokio::test]
async fn test_integrity() {
    let dna_compat = DnaCompat::fake();
    let pack_dna = move |path| {
        let dna_compat = dna_compat.clone();
        async move {
            let mut cmd = Command::cargo_bin("hc-dna").unwrap();
            let cmd = cmd.args(["pack", path]);
            cmd.assert().success();
            let dna_path = PathBuf::from(format!("{}/integrity dna.dna", path));
            let original_dna = read_dna(&dna_path).unwrap();
            original_dna
                .into_dna_file(DnaModifiersOpt::none(), dna_compat.clone())
                .await
                .unwrap()
        }
    };
    let (integrity_dna, integrity_dna_hash) = pack_dna("tests/fixtures/my-app/dnas/dna3").await;
    let (coordinator_dna, coordinator_dna_hash) = pack_dna("tests/fixtures/my-app/dnas/dna4").await;

    assert_eq!(integrity_dna_hash, coordinator_dna_hash);

    integrity_dna.verify_hash().unwrap();
    coordinator_dna.verify_hash().unwrap();

    assert_eq!(integrity_dna.code().len(), 1);
    assert_eq!(coordinator_dna.code().len(), 2);

    assert_eq!(
        integrity_dna.get_wasm_for_zome(&"zome1".into()).unwrap(),
        coordinator_dna.get_wasm_for_zome(&"zome1".into()).unwrap()
    );
    assert_ne!(
        integrity_dna.get_wasm_for_zome(&"zome1".into()).unwrap(),
        coordinator_dna.get_wasm_for_zome(&"zome2".into()).unwrap()
    );

    let integrity_def = integrity_dna.dna_def().clone();
    let mut coordinator_def = coordinator_dna.dna_def().clone();

    assert_eq!(
        integrity_def.get_wasm_zome(&"zome1".into()).unwrap(),
        coordinator_def.get_wasm_zome(&"zome1".into()).unwrap()
    );
    assert_ne!(
        integrity_def.get_wasm_zome(&"zome1".into()).unwrap(),
        coordinator_def.get_wasm_zome(&"zome2".into()).unwrap()
    );

    assert_eq!(
        integrity_def.integrity_zomes,
        coordinator_def.integrity_zomes
    );
    assert_eq!(coordinator_def.integrity_zomes.len(), 1);
    assert_eq!(coordinator_def.coordinator_zomes.len(), 1);
    assert_eq!(integrity_def.coordinator_zomes.len(), 0);

    assert_eq!(
        DnaHash::with_data_sync(&integrity_def),
        DnaHash::with_data_sync(&coordinator_def),
    );
    assert_eq!(
        DnaDefHashed::from_content_sync(integrity_def.clone()),
        DnaDefHashed::from_content_sync(coordinator_def.clone()),
    );

    assert_ne!(integrity_def, coordinator_def,);

    coordinator_def.coordinator_zomes.clear();

    assert_eq!(integrity_def, coordinator_def,);
}

#[tokio::test]
/// Test that a manifest with multiple integrity zomes and dependencies parses
/// to the correct dna file.
async fn test_multi_integrity() {
    let dna_compat = DnaCompat::fake();
    let dna_compat_clone = dna_compat.clone();

    let pack_dna = |path| async move {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(["pack", path]);
        cmd.assert().success();
        let dna_path = PathBuf::from(format!("{}/multi integrity dna.dna", path));
        let original_dna = read_dna(&dna_path).unwrap();
        original_dna
            .into_dna_file(DnaModifiersOpt::none(), dna_compat_clone.clone())
            .await
            .unwrap()
    };

    let (dna, _) = pack_dna("tests/fixtures/my-app/dnas/dna5").await;

    // The actual wasm hashes of the fake zomes.
    let wasm_hash = WasmHash::from_raw_39_panicky(vec![
        132, 42, 36, 217, 5, 131, 6, 203, 162, 51, 6, 34, 63, 247, 21, 77, 60, 106, 98, 53, 59, 98,
        172, 222, 143, 105, 210, 10, 5, 56, 152, 102, 178, 159, 162, 69, 249, 162, 67,
    ]);
    let wasm_hash2 = WasmHash::from_raw_39_panicky(vec![
        132, 42, 36, 235, 225, 55, 255, 141, 140, 72, 148, 154, 141, 124, 248, 185, 142, 62, 218,
        220, 85, 73, 201, 54, 10, 30, 191, 206, 93, 108, 142, 140, 201, 164, 225, 20, 241, 98, 16,
    ]);

    // Create the expected dependencies on the coordinator zomes.
    let s = "2022-02-11T23:05:19.470323Z";
    let origin_time = Timestamp::from_str(s).unwrap();
    let expected = DnaDef {
        name: "multi integrity dna".into(),
        modifiers: DnaModifiers {
            network_seed: "00000000-0000-0000-0000-000000000000".into(),
            properties: ().try_into().unwrap(),
            origin_time,
            quantum_time: Duration::from_secs(5 * 60),
        },
        compatibility: dna_compat,
        integrity_zomes: vec![
            (
                "zome1".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                    preserialized_path: None,
                })
                .into(),
            ),
            (
                "zome2".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                    preserialized_path: None,
                })
                .into(),
            ),
        ],
        coordinator_zomes: vec![
            (
                "zome3".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into()],
                    preserialized_path: None,
                })
                .into(),
            ),
            (
                "zome4".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into(), "zome2".into()],
                    preserialized_path: None,
                })
                .into(),
            ),
        ],
    };
    assert_eq!(
        dna.dna_def().integrity_zomes[0]
            .1
            .as_any_zome_def()
            .dependencies(),
        &[]
    );
    assert_eq!(
        dna.dna_def().integrity_zomes[1]
            .1
            .as_any_zome_def()
            .dependencies(),
        &[]
    );
    assert_eq!(
        dna.dna_def().coordinator_zomes[0]
            .1
            .as_any_zome_def()
            .dependencies(),
        &["zome1".into()]
    );
    assert_eq!(
        dna.dna_def().coordinator_zomes[1]
            .1
            .as_any_zome_def()
            .dependencies(),
        &["zome1".into(), "zome2".into()]
    );
    assert_eq!(*dna.dna_def(), expected);
}

#[test]
fn test_all_dna_manifests_match_schema() {
    let schema = load_schema("dna-manifest");

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.eq("dna.yaml") {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = serde_yaml::from_str(manifest_content.as_str()).unwrap();

            validate_schema(&schema, &manifest, file_name.as_ref());
        }
    }
}

#[test]
fn test_default_dna_manifest_matches_schema() {
    let default_manifest = DnaManifest::current(
        "test-dna".to_string(),
        Some("00000000-0000-0000-0000-000000000000".to_string()),
        None,
        Timestamp::now().into(),
        Default::default(),
        vec![],
        vec![],
    );

    let default_manifest: Value =
        serde_yaml::from_str(&serde_yaml::to_string(&default_manifest).unwrap()).unwrap();

    let schema = load_schema("dna-manifest");
    validate_schema(&schema, &default_manifest, "default manifest");
}

#[test]
fn test_all_app_manifests_match_schema() {
    let schema = load_schema("happ-manifest");

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.eq("happ.yaml") {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = serde_yaml::from_str(manifest_content.as_str()).unwrap();

            validate_schema(&schema, &manifest, file_name.as_ref());
        }
    }
}

#[test]
fn test_default_app_manifest_matches_schema() {
    let role = AppRoleManifest::sample("sample-role".into());
    let default_manifest: AppManifest = AppManifestCurrentBuilder::default()
        .name("test-app".to_string())
        .description(None)
        .roles(vec![role])
        .build()
        .unwrap()
        .into();

    let default_manifest: Value =
        serde_yaml::from_str(&serde_yaml::to_string(&default_manifest).unwrap()).unwrap();

    let schema = load_schema("happ-manifest");
    validate_schema(&schema, &default_manifest, "default manifest");
}

#[test]
fn test_all_web_app_manifests_match_schema() {
    let schema = load_schema("web-happ-manifest");

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.eq("web-happ.yaml") {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = serde_yaml::from_str(manifest_content.as_str()).unwrap();

            validate_schema(&schema, &manifest, file_name.as_ref());
        }
    }
}

#[test]
fn test_default_web_app_manifest_matches_schema() {
    let default_manifest = WebAppManifest::current("test-web-app".to_string());

    let default_manifest: Value =
        serde_yaml::from_str(&serde_yaml::to_string(&default_manifest).unwrap()).unwrap();

    let schema = load_schema("web-happ-manifest");
    validate_schema(&schema, &default_manifest, "default manifest");
}

fn load_schema(schema_name: &str) -> JSONSchema {
    let schema_content =
        ffs::sync::read_to_string(format!("./schema/{}.schema.json", schema_name)).unwrap();
    let schema: Value = serde_json::from_str(schema_content.as_str()).unwrap();
    let schema = JSONSchema::compile(&schema).expect("Schema should be valid");
    schema
}

fn validate_schema(schema: &JSONSchema, manifest: &Value, context: &str) {
    let result = schema.validate(manifest);
    if let Err(errors) = result {
        for error in errors {
            println!("Validation error: {}", error);
            println!("At path: {}", error.instance_path);
        }
        panic!("There were schema validation errors for {}", context);
    }
}
