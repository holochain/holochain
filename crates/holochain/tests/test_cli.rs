#![cfg(test)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempdir::TempDir;

#[test]
// (david.b) [D-01034] these take minutes to run in nix/CI - disable until fixed
fn first_experience_with_holochain_is_a_friendly_one() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("missing-config.toml");
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
}

#[test]
// (david.b) [D-01034] these take minutes to run in nix/CI - disable until fixed
fn malformed_toml_error_is_friendly() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("malformed-config.toml");
    std::fs::write(&path, "{{ totally [ not ( valid toml").unwrap();
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
    cmd.assert()
        .append_context("reason", "output contains the wrong reason for error")
        .stdout(predicate::str::contains("expected a table key"));
}

#[test]
// (david.b) [D-01034] these take minutes to run in nix/CI - disable until fixed
fn invalid_config_error_is_friendly() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("malformed-config.toml");
    std::fs::write(&path, "[valid]\nbut=\"wrong\"").unwrap();
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
    cmd.assert()
        .append_context("reason", "output contains the wrong reason for error")
        .stdout(predicate::str::contains("missing field"));
}
