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
