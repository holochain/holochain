//! Integration tests for `hc export-ts-bindings`.
//!
//! These live under `tests/`, not as `hc`'s lib unit tests, so that
//! `assert_cmd::Command::cargo_bin("hc")` resolves through Cargo's
//! `CARGO_BIN_EXE_hc` environment variable (set only for integration-test
//! and bench binary targets) instead of guessing `target/debug/hc`. That
//! way `cargo test -p holochain_cli` rebuilds the `hc` binary these tests
//! invoke, with the features the test run enables, rather than exercising
//! whatever binary happened to be on disk already.

use assert_cmd::Command;
use predicates::prelude::*;

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn export_ts_bindings_writes_the_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bindings");

    Command::cargo_bin("hc")
        .unwrap()
        .args(["export-ts-bindings", "--out-dir"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript bindings written to"));

    let admin = read(&out.join("api/admin/types.ts"));
    assert!(admin.contains("export type AdminRequest"));
    assert!(admin.contains("@public"), "release tags must be injected");

    let shared = read(&out.join("types.ts"));
    assert!(shared.contains("export type HoloHash = Uint8Array;"));
    // 64-bit integers are `number`, not `bigint`: the dialect is fixed in
    // code and must not depend on the environment.
    assert!(shared.contains("export type Timestamp = number;"));
    assert!(!shared.contains("bigint"));
    // NodeNext resolution: relative imports carry a `.js` extension.
    assert!(admin.contains(".js\""), "imports must use .js extensions");
}

#[test]
fn export_ts_bindings_replaces_a_stale_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bindings");
    std::fs::create_dir_all(&out).unwrap();
    let stale = out.join("stale.ts");
    std::fs::write(&stale, "export type Stale = never;\n").unwrap();

    // A directory holding only `.ts` files looks like a binding tree, so
    // this must succeed without `--force`.
    Command::cargo_bin("hc")
        .unwrap()
        .args(["export-ts-bindings", "-o"])
        .arg(&out)
        .assert()
        .success();

    assert!(!stale.exists(), "previous contents must be replaced");
    assert!(out.join("types.ts").exists());
}

#[test]
fn export_ts_bindings_defaults_to_bindings_in_cwd() {
    let tmp = tempfile::tempdir().unwrap();

    Command::cargo_bin("hc")
        .unwrap()
        .current_dir(tmp.path())
        .arg("export-ts-bindings")
        .assert()
        .success();

    assert!(tmp.path().join("bindings/types.ts").exists());
}

#[test]
fn export_ts_bindings_refuses_to_replace_the_working_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let canary = tmp.path().join("keep.txt");
    std::fs::write(&canary, "keep").unwrap();

    for out_dir in [".", "..", tmp.path().to_str().unwrap()] {
        Command::cargo_bin("hc")
            .unwrap()
            .current_dir(tmp.path())
            .args(["export-ts-bindings", "--out-dir", out_dir])
            .assert()
            .failure()
            .stderr(predicate::str::contains("refusing"));
    }
    assert!(canary.exists(), "nothing may be deleted");
}

#[test]
fn export_ts_bindings_refuses_unrecognized_contents_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bindings");
    std::fs::create_dir_all(&out).unwrap();
    let random_file = out.join("random-file.bin");
    std::fs::write(&random_file, [0u8, 1, 2, 3]).unwrap();
    std::fs::create_dir_all(out.join(".git")).unwrap();

    Command::cargo_bin("hc")
        .unwrap()
        .args(["export-ts-bindings", "--out-dir"])
        .arg(&out)
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing"))
        .stderr(predicate::str::contains("--force"));

    assert!(
        random_file.exists(),
        "an out-dir that isn't a binding tree must be left untouched without --force"
    );
    assert!(!out.join("types.ts").exists());
}

#[test]
fn export_ts_bindings_force_replaces_unrecognized_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bindings");
    std::fs::create_dir_all(&out).unwrap();
    let random_file = out.join("random-file.bin");
    std::fs::write(&random_file, [0u8, 1, 2, 3]).unwrap();

    Command::cargo_bin("hc")
        .unwrap()
        .args(["export-ts-bindings", "--out-dir"])
        .arg(&out)
        .arg("--force")
        .assert()
        .success();

    assert!(!random_file.exists(), "--force must replace the contents");
    assert!(out.join("types.ts").exists());
}

#[cfg(feature = "unstable-countersigning")]
#[test]
fn export_ts_bindings_includes_countersigning_app_api_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bindings");

    Command::cargo_bin("hc")
        .unwrap()
        .args(["export-ts-bindings", "--out-dir"])
        .arg(&out)
        .assert()
        .success();

    let app = read(&out.join("api/app/types.ts"));
    // Wire tags are snake_case: the request variant's `#[serde(tag)]`
    // value, not the Rust variant name.
    assert!(app.contains("\"get_countersigning_session_state\""));
}
