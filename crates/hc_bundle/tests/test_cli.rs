use assert_cmd::prelude::*;
use holochain_types::web_app::WebAppManifest;
use holochain_types::{prelude::*, web_app::WebAppBundle};
use holochain_util::ffs;
use mr_bundle::FileSystemBundler;
use schemars::JsonSchema;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

async fn read_app(path: &Path) -> anyhow::Result<AppBundle> {
    Ok(FileSystemBundler::load_from::<AppManifest>(path)
        .await
        .map(AppBundle::from)?)
}

async fn read_dna(path: &Path) -> anyhow::Result<DnaBundle> {
    Ok(FileSystemBundler::load_from::<ValidatedDnaManifest>(path)
        .await
        .map(DnaBundle::from)?)
}

async fn read_web_app(path: &Path) -> anyhow::Result<WebAppBundle> {
    Ok(FileSystemBundler::load_from::<WebAppManifest>(path)
        .await
        .map(WebAppBundle::from)?)
}

#[tokio::test]
async fn round_trip() {
    {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/dnas/dna1"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/dnas/dna2"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-app"));
        let cmd = cmd.args(["pack", "tests/fixtures/my-app/"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-web-app"));
        let cmd = cmd.args(["pack", "tests/fixtures/web-app/"]);
        cmd.assert().success();
    }

    let web_app_path = PathBuf::from("tests/fixtures/web-app/fixture-web-app.webhapp");
    let app_path = PathBuf::from("tests/fixtures/my-app/fixture-app.happ");
    let dna1_path = PathBuf::from("tests/fixtures/my-app/dnas/dna1/a dna.dna");
    let dna2_path = PathBuf::from("tests/fixtures/my-app/dnas/dna2/another dna.dna");

    let _original_web_happ = read_web_app(&web_app_path).await.unwrap();
    let _original_happ = read_app(&app_path).await.unwrap();
    let _original_dna1 = read_dna(&dna1_path).await.unwrap();
    let _original_dna2 = read_dna(&dna2_path).await.unwrap();
}

#[tokio::test]
#[cfg_attr(
    target_os = "macos",
    ignore = "don't use system sha256sum - use a rust library"
)]
async fn test_packed_hash_consistency() {
    let mut i = 0;
    let mut hash = None;
    while i < 5 {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
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
    let pack_dna = |path| async move {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
        let cmd = cmd.args(["pack", path]);
        cmd.assert().success();
        let mut dna_path = PathBuf::from(path);
        dna_path.push("integrity dna.dna");
        let original_dna = read_dna(&dna_path).await.unwrap();
        original_dna
            .into_dna_file(DnaModifiersOpt::none())
            .await
            .unwrap()
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

/// Test that a manifest with multiple integrity zomes and dependencies parses
/// to the correct dna file.
#[tokio::test]
#[cfg_attr(target_os = "windows", ignore = "theres a hash mismatch - check crlf?")]
#[cfg(not(feature = "unstable-migration"))]
async fn test_multi_integrity() {
    let pack_dna = |path| async move {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
        let cmd = cmd.args(["pack", path]);
        cmd.assert().success();
        let dna_path = PathBuf::from(format!("{path}/multi integrity dna.dna"));
        let original_dna = read_dna(&dna_path).await.unwrap();
        original_dna
            .into_dna_file(DnaModifiersOpt::none())
            .await
            .unwrap()
    };

    let (dna, _) = pack_dna("tests/fixtures/my-app/dnas/dna5").await;

    // The actual wasm hashes of the fake zomes.
    let wasm_hash = WasmHash::from_raw_39(vec![
        132, 42, 36, 217, 5, 131, 6, 203, 162, 51, 6, 34, 63, 247, 21, 77, 60, 106, 98, 53, 59, 98,
        172, 222, 143, 105, 210, 10, 5, 56, 152, 102, 178, 159, 162, 69, 249, 162, 67,
    ]);
    let wasm_hash2 = WasmHash::from_raw_39(vec![
        132, 42, 36, 235, 225, 55, 255, 141, 140, 72, 148, 154, 141, 124, 248, 185, 142, 62, 218,
        220, 85, 73, 201, 54, 10, 30, 191, 206, 93, 108, 142, 140, 201, 164, 225, 20, 241, 98, 16,
    ]);

    // Create the expected dependencies on the coordinator zomes.
    let expected = DnaDef {
        name: "multi integrity dna".into(),
        modifiers: DnaModifiers {
            network_seed: "00000000-0000-0000-0000-000000000000".into(),
            properties: ().try_into().unwrap(),
        },
        integrity_zomes: vec![
            (
                "zome1".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                })
                .into(),
            ),
            (
                "zome2".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                })
                .into(),
            ),
        ],
        coordinator_zomes: vec![
            (
                "zome3".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into()],
                })
                .into(),
            ),
            (
                "zome4".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into(), "zome2".into()],
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

/// Test that a manifest with multiple integrity zomes and dependencies parses
/// to the correct dna file.
#[tokio::test]
#[cfg_attr(target_os = "windows", ignore = "theres a hash mismatch - check crlf?")]
#[cfg(feature = "unstable-migration")]
async fn test_multi_integrity() {
    let pack_dna = |path| async move {
        let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
        let cmd = cmd.args(["pack", path]);
        cmd.assert().success();
        let dna_path = PathBuf::from(format!("{path}/multi integrity dna unstable-migration.dna"));
        let original_dna = read_dna(&dna_path).await.unwrap();
        original_dna
            .into_dna_file(DnaModifiersOpt::none())
            .await
            .unwrap()
    };

    let (dna, _) = pack_dna("tests/fixtures/my-app/dnas/dna-unstable-migration").await;

    // The actual wasm hashes of the fake zomes.
    let wasm_hash = WasmHash::from_raw_39(vec![
        132, 42, 36, 217, 5, 131, 6, 203, 162, 51, 6, 34, 63, 247, 21, 77, 60, 106, 98, 53, 59, 98,
        172, 222, 143, 105, 210, 10, 5, 56, 152, 102, 178, 159, 162, 69, 249, 162, 67,
    ]);
    let wasm_hash2 = WasmHash::from_raw_39(vec![
        132, 42, 36, 235, 225, 55, 255, 141, 140, 72, 148, 154, 141, 124, 248, 185, 142, 62, 218,
        220, 85, 73, 201, 54, 10, 30, 191, 206, 93, 108, 142, 140, 201, 164, 225, 20, 241, 98, 16,
    ]);

    // Create the expected dependencies on the coordinator zomes.
    let lineage = vec![
        DnaHash::try_from_raw_39(
            holo_hash_decode_unchecked("uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm")
                .unwrap(),
        )
        .unwrap(),
        DnaHash::try_from_raw_39(
            holo_hash_decode_unchecked("uhC0k39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KNZ0dUH")
                .unwrap(),
        )
        .unwrap(),
    ];
    let expected = DnaDef {
        name: "multi integrity dna unstable-migration".into(),
        modifiers: DnaModifiers {
            network_seed: "00000000-0000-0000-0000-000000000000".into(),
            properties: ().try_into().unwrap(),
        },
        integrity_zomes: vec![
            (
                "zome1".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                })
                .into(),
            ),
            (
                "zome2".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash.clone(),
                    dependencies: vec![],
                })
                .into(),
            ),
        ],
        coordinator_zomes: vec![
            (
                "zome3".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into()],
                })
                .into(),
            ),
            (
                "zome4".into(),
                ZomeDef::Wasm(WasmZomeDef {
                    wasm_hash: wasm_hash2.clone(),
                    dependencies: vec!["zome1".into(), "zome2".into()],
                })
                .into(),
            ),
        ],
        lineage: lineage.into_iter().collect(),
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

fn pack_dna_hash_fixture() -> (tempfile::TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/my-app/dnas/dna1");
    let dna_dir = temp_dir.path().join("dna1");
    fs::create_dir_all(dna_dir.join("zomes")).unwrap();

    for relative_path in ["dna.yaml", "zomes/zome11.wasm", "zomes/zome12.wasm"] {
        fs::copy(source_dir.join(relative_path), dna_dir.join(relative_path)).unwrap();
    }

    let dna_path = dna_dir.join("a dna.dna");
    assert!(dna_path.is_absolute());
    assert!(!dna_path.exists());

    let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
    cmd.arg("pack").arg(&dna_dir);
    cmd.assert().success();

    assert!(dna_path.is_file());

    (temp_dir, dna_path)
}

fn dna_hash_command(dna_path: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo_bin!("hc-dna"));
    cmd.arg("hash").arg(dna_path);
    cmd
}

fn dna_hash_cli(dna_path: &Path, args: &[&str]) -> String {
    let mut cmd = dna_hash_command(dna_path);
    cmd.args(args);
    let stdout = cmd.assert().success().get_output().stdout.clone();
    // Normalize Windows/linux line endings
    String::from_utf8_lossy(&stdout).replace(['\r', '\n'], "")
}

fn dna_hash_cli_with_role_settings(
    dna_path: &Path,
    network_seed: Option<&str>,
    role_settings: &Path,
) -> String {
    let mut cmd = dna_hash_command(dna_path);
    if let Some(network_seed) = network_seed {
        cmd.arg("--network-seed").arg(network_seed);
    }
    cmd.arg("--role-settings").arg(role_settings);
    let stdout = cmd.assert().success().get_output().stdout.clone();
    String::from_utf8_lossy(&stdout).replace(['\r', '\n'], "")
}

fn dna_hash_cli_failure(dna_path: &Path, role_settings: &Path) -> String {
    let mut cmd = dna_hash_command(dna_path);
    cmd.arg("--role-settings").arg(role_settings);
    let stderr = cmd.assert().failure().get_output().stderr.clone();
    String::from_utf8_lossy(&stderr).into_owned()
}

#[tokio::test]
#[cfg_attr(target_os = "windows", ignore = "theres a hash mismatch - check crlf?")]
async fn hash_dna_function() {
    let (_temp_dir, dna_path) = pack_dna_hash_fixture();
    let expected = "uhC0kMpN6EzEhjaPP-MWeJi1cH2Zyw7OYEDEekSnjWE85WgCnIvEG";
    let actual = dna_hash_cli(&dna_path, &[]);
    assert_eq!(expected, actual, "Expected: {expected}\nActual: {actual}");
}

#[tokio::test]
#[cfg_attr(target_os = "windows", ignore = "theres a hash mismatch - check crlf?")]
async fn hash_dna_with_modifier_overrides() {
    let (_temp_dir, dna_path) = pack_dna_hash_fixture();
    let base = dna_hash_cli(&dna_path, &[]);

    // --network-seed alone changes the hash, deterministically on every OS.
    let seed_hash = dna_hash_cli(&dna_path, &["--network-seed", "hc-dna-hash-test-seed"]);
    assert_ne!(base, seed_hash);
    let expected_seed_hash = "uhC0kZR2x_xAxE4_wypMnRPjwhk4DeG1dnOGphZpL1lBRAnqoYLoJ";
    assert_eq!(expected_seed_hash, seed_hash);

    let short_seed_hash = dna_hash_cli(&dna_path, &["-s", "hc-dna-hash-test-seed"]);
    assert_eq!(expected_seed_hash, short_seed_hash);
    assert_eq!(seed_hash, short_seed_hash);

    // The CLI must agree with the library path used at install time.
    let bundle = read_dna(&dna_path).await.unwrap();
    let modifiers = DnaModifiersOpt::none().with_network_seed("hc-dna-hash-test-seed".into());
    let library_hash = bundle
        .into_dna_file(modifiers)
        .await
        .unwrap()
        .0
        .dna_hash()
        .to_string();
    assert_eq!(library_hash, seed_hash);

    // --role-settings overrides seed and properties.
    let role_settings =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dna-role-settings.yaml");
    let settings_hash = dna_hash_cli_with_role_settings(&dna_path, None, &role_settings);
    assert_ne!(base, settings_hash);
    assert_ne!(seed_hash, settings_hash);
    assert_eq!(
        "uhC0kvdsQLPWXzSUqwIC1Gr423E_oB9IF5RsizmRTUPv_fyTILYTZ",
        settings_hash
    );

    let bundle = read_dna(&dna_path).await.unwrap();
    let properties = YamlProperties::new(yaml_serde::from_str("foo: bar").unwrap());
    let modifiers = DnaModifiersOpt::none()
        .with_network_seed("hc-dna-hash-test-seed".into())
        .with_properties(properties)
        .serialized()
        .unwrap();
    let library_hash = bundle
        .into_dna_file(modifiers)
        .await
        .unwrap()
        .0
        .dna_hash()
        .to_string();
    assert_eq!(library_hash, settings_hash);

    // When both are given, the role settings file wins (install-time precedence).
    let both_hash =
        dna_hash_cli_with_role_settings(&dna_path, Some("some-other-seed"), &role_settings);
    assert_eq!(settings_hash, both_hash);
}

#[test]
fn hash_dna_rejects_missing_role_settings_file() {
    let (temp_dir, dna_path) = pack_dna_hash_fixture();
    let missing_path = temp_dir.path().join("missing-role-settings.yaml");

    let stderr = dna_hash_cli_failure(&dna_path, &missing_path);

    assert!(
        stderr.contains("missing-role-settings.yaml"),
        "stderr did not name the missing role settings file: {stderr}"
    );
}

#[test]
fn hash_dna_rejects_malformed_role_settings_yaml() {
    let (temp_dir, dna_path) = pack_dna_hash_fixture();
    let settings_path = temp_dir.path().join("malformed-role-settings.yaml");
    fs::write(&settings_path, "modifiers: [unterminated\n").unwrap();

    let stderr = dna_hash_cli_failure(&dna_path, &settings_path);

    assert!(
        stderr.contains("Failed to parse the role settings file"),
        "stderr did not explain that the role settings YAML is malformed: {stderr}"
    );
}

#[test]
fn hash_dna_rejects_role_settings_without_modifiers() {
    let (temp_dir, dna_path) = pack_dna_hash_fixture();
    let settings_path = temp_dir.path().join("role-settings-without-modifiers.yaml");
    fs::write(&settings_path, "type: provisioned\nmembrane_proof: ~\n").unwrap();

    let stderr = dna_hash_cli_failure(&dna_path, &settings_path);

    assert!(
        stderr.contains("does not contain a `modifiers` block"),
        "stderr did not explain that the modifiers block is required: {stderr}"
    );
}

#[test]
fn test_all_dna_manifests_match_schema() {
    let schema = get_schema::<DnaManifest>();

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        let should_check = if cfg!(feature = "unstable-migration") {
            entry
                .path()
                .parent()
                .unwrap()
                .ends_with("dna-unstable-migration")
        } else {
            !entry
                .path()
                .parent()
                .unwrap()
                .ends_with("dna-unstable-migration")
        };
        if file_name.eq("dna.yaml") && should_check {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = yaml_serde::from_str(manifest_content.as_str()).unwrap();

            validate_schema(&schema, &manifest, file_name.as_ref());
        }
    }
}

#[test]
#[cfg(not(feature = "unstable-migration"))]
fn test_default_dna_manifest_matches_schema() {
    let default_manifest = DnaManifest::current(
        "test-dna".to_string(),
        Some("00000000-0000-0000-0000-000000000000".to_string()),
        None,
        vec![],
        vec![],
    );

    let default_manifest: Value =
        yaml_serde::from_str(&yaml_serde::to_string(&default_manifest).unwrap()).unwrap();

    let schema = get_schema::<DnaManifest>();
    validate_schema(&schema, &default_manifest, "default manifest");
}

#[test]
#[cfg(feature = "unstable-migration")]
fn test_default_dna_manifest_matches_schema() {
    let default_manifest = DnaManifest::current(
        "test-dna".to_string(),
        Some("00000000-0000-0000-0000-000000000000".to_string()),
        None,
        vec![],
        vec![],
        vec![],
    );

    let default_manifest: Value =
        yaml_serde::from_str(&yaml_serde::to_string(&default_manifest).unwrap()).unwrap();

    let schema = get_schema::<DnaManifest>();
    validate_schema(&schema, &default_manifest, "default manifest");
}

#[test]
fn test_all_app_manifests_match_schema() {
    let schema = get_schema::<AppManifest>();

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.eq("happ.yaml") {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = yaml_serde::from_str(manifest_content.as_str()).unwrap();

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
        yaml_serde::from_str(&yaml_serde::to_string(&default_manifest).unwrap()).unwrap();

    let schema = get_schema::<AppManifest>();
    validate_schema(&schema, &default_manifest, "default manifest");
}

#[test]
fn test_all_web_app_manifests_match_schema() {
    let schema = get_schema::<WebAppManifest>();

    for entry in WalkDir::new("./tests/fixtures")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.eq("web-happ.yaml") {
            let manifest_content = ffs::sync::read_to_string(entry.path()).unwrap();
            let manifest: Value = yaml_serde::from_str(manifest_content.as_str()).unwrap();

            validate_schema(&schema, &manifest, file_name.as_ref());
        }
    }
}

#[test]
fn test_default_web_app_manifest_matches_schema() {
    let default_manifest = WebAppManifest::current("test-web-app".to_string());

    let default_manifest: Value =
        yaml_serde::from_str(&yaml_serde::to_string(&default_manifest).unwrap()).unwrap();

    let schema = get_schema::<WebAppManifest>();
    validate_schema(&schema, &default_manifest, "default manifest");
}

fn get_schema<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);
    serde_json::to_value(&schema).unwrap()
}

fn validate_schema(schema: &Value, manifest: &Value, context: &str) {
    let result = jsonschema::validate(schema, manifest);
    if let Err(error) = result {
        println!("Validation error: {error}");

        panic!("There were schema validation errors for {context}");
    }
}
