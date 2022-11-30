#![cfg(test)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn first_experience_with_holochain_is_a_friendly_one() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("missing-config.yml");
    let mut cmd = Command::cargo_bin("holochain").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
}

#[test]
fn malformed_toml_error_is_friendly() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("malformed-config.yml");
    std::fs::write(&path, "{{ totally [ not ( valid yaml").unwrap();
    let mut cmd = Command::cargo_bin("holochain").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
    cmd.assert()
        .append_context("reason", "output contains the wrong reason for error")
        .stdout(predicate::str::contains("invalid type"));
}

#[test]
fn invalid_config_error_is_friendly() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("malformed-config.yml");
    std::fs::write(&path, "valid:\n  but: wrong").unwrap();
    let mut cmd = Command::cargo_bin("holochain").unwrap();
    let cmd = cmd.args(&["-c", &path.display().to_string()]);
    cmd.assert().failure().code(predicate::eq(42));
    cmd.assert()
        .append_context("reason", "output doesn't contain the word \"please\"")
        .stdout(predicate::str::is_match("[Pp]lease").unwrap());
    cmd.assert()
        .append_context("reason", "output contains the wrong reason for error")
        .stdout(predicate::str::contains("missing field"));
}
