use assert_cmd::Command;
use predicates::prelude::*;
use uuid::Uuid;

#[test]
fn cross_process() {
    let dir = tempdir::TempDir::new("tmp_sock").expect("Failed to create tmp dir");
    let trace_id = Uuid::new_v4();
    let dir2 = dir.path().to_owned();
    let j = std::thread::spawn(move || {
        let mut cmd = Command::cargo_bin("tracing_spike").expect("failed to start bin");
        let assert = cmd
            .arg("--structured")
            .arg("Json")
            .arg("--trace-id")
            .arg(trace_id.to_string())
            .arg("--server")
            .arg(dir2)
            .env("CUSTOM_FILTER", "trace")
            .assert();
        assert.success();
    });
    std::thread::sleep(std::time::Duration::from_secs(2));
    let mut cmd = Command::cargo_bin("tracing_spike").expect("failed to start bin");
    let assert = cmd
        .arg("--structured")
        .arg("Json")
        .env("CUSTOM_FILTER", "trace")
        .arg("--client")
        .arg(dir.path())
        .assert();
    let contains_trace = predicate::str::contains(trace_id.to_string());
    assert.success().stdout(contains_trace);
    j.join().expect("failed to join thread");
}
