use assert_cmd::prelude::*;
use holochain_types::{prelude::*, web_app::WebAppBundle};
use holochain_util::ffs;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

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
        let cmd = cmd.args(&["pack", "tests/fixtures/my-app/dnas/dna1"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(&["pack", "tests/fixtures/my-app/dnas/dna2"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-app").unwrap();
        let cmd = cmd.args(&["pack", "tests/fixtures/my-app/"]);
        cmd.assert().success();
    }
    {
        let mut cmd = Command::cargo_bin("hc-web-app").unwrap();
        let cmd = cmd.args(&["pack", "tests/fixtures/web-app/"]);
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
async fn test_integrity() {
    let pack_dna = |path| async move {
        let mut cmd = Command::cargo_bin("hc-dna").unwrap();
        let cmd = cmd.args(&["pack", path]);
        cmd.assert().success();
        let dna_path = PathBuf::from(format!("{}/integrity dna.dna", path));
        let original_dna = read_dna(&dna_path).unwrap();
        original_dna.into_dna_file(None, None).await.unwrap()
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
