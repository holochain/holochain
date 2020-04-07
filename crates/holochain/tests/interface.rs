use assert_cmd::prelude::*;
use std::{
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
};

fn check_started(started: Result<Option<ExitStatus>, std::io::Error>, holochain: &mut Child) {
    if let Ok(Some(status)) = started {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(ref mut so) = holochain.stdout {
            so.read_to_string(&mut stdout).ok();
        }
        if let Some(ref mut se) = holochain.stderr {
            se.read_to_string(&mut stderr).ok();
        }
        panic!(
            "Holochain failed to start. status: {:?}, stdout: {}, stderr: {}",
            status, stdout, stderr
        );
    }
}

#[test]
fn call_admin() {
    let mut cmd = Command::cargo_bin("holochain-2020").unwrap();
    cmd.arg("--admin");
    cmd.arg("--structured");
    cmd.arg("--websocket-example");
    cmd.arg("9000");
    cmd.env("RUST_LOG", "debug");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let started = holochain.try_wait();
    check_started(started, &mut holochain);

    holochain.kill().expect("Failed to kill holochain");
}
