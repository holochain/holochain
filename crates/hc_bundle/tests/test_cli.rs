use assert_cmd::prelude::*;
use holochain_types::prelude::*;
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

    let app_path = PathBuf::from("tests/fixtures/my-app.happ");
    let dna1_path = PathBuf::from("tests/fixtures/my-app/dnas/dna1.dna");
    let dna2_path = PathBuf::from("tests/fixtures/my-app/dnas/dna2.dna");

    let _original_happ = read_app(&app_path).unwrap();
    let _original_dna1 = read_dna(&dna1_path).unwrap();
    let _original_dna2 = read_dna(&dna2_path).unwrap();
}
