use assert_cmd::Command;
use predicates::prelude::*;
use uuid::Uuid;

#[test]
fn cross_process() {
    let trace_id = Uuid::new_v4();
    let mut cmd = Command::cargo_bin("tracing_spike").expect("failed to start bin");
    let assert = cmd
        .arg("--structured")
        .arg("Json")
        .arg("--trace-id")
        .arg(trace_id.to_string())
        .env("CUSTOM_FILTER", "trace")
        .assert();
    let assert = assert.success();
    let out = assert.get_output();
    let mut cmd = Command::cargo_bin("tracing_spike").expect("failed to start bin");
    let assert = cmd
        .arg("--structured")
        .arg("Json")
        .env("CUSTOM_FILTER", "trace")
        .write_stdin(out.stdout.clone())
        .assert();
    let contains_trace = predicate::str::contains(trace_id.to_string());
    assert.success().stdout(contains_trace);
}
