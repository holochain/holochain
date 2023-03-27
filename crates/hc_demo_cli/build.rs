fn main() {
    let inception = std::env::var_os("HC_DEMO_CLI_INCEPTION").is_some();

    println!("cargo:warning=HC_DEMO_CLI inception: {inception}");

    if inception {
        return;
    }

    let cargo_cmd = std::env::var_os("CARGO");
    let cargo_cmd = cargo_cmd.as_deref().unwrap_or_else(|| "cargo".as_ref());

    let out_dir = std::env::var_os("OUT_DIR").expect("have out dir");

    let integrity_wasm = match std::env::var_os("HC_DEMO_CLI_INTEGRITY_WASM") {
        Some(wasm) => std::path::PathBuf::from(wasm),
        None => build(cargo_cmd, "build_integrity_wasm"),
    };

    let mut integrity_out = std::path::PathBuf::from(&out_dir);
    integrity_out.push("integrity.wasm");
    copy_wasm(&integrity_wasm, &integrity_out);

    let coordinator_wasm = match std::env::var_os("HC_DEMO_CLI_COORDINATOR_WASM") {
        Some(wasm) => std::path::PathBuf::from(wasm),
        None => build(cargo_cmd, "build_coordinator_wasm"),
    };

    let mut coordinator_out = std::path::PathBuf::from(&out_dir);
    coordinator_out.push("coordinator.wasm");
    copy_wasm(&coordinator_wasm, &coordinator_out);
}

fn copy_wasm(from: &std::path::Path, to: &std::path::Path) {
    println!("cargo:warning=HC_DEMO_CLI copy wasm: from: {from:?}, to: {to:?}");
    std::fs::copy(from, to).unwrap();
}

fn build(cargo_cmd: &std::ffi::OsStr, tgt: &str) -> std::path::PathBuf {
    let target_dir = env!("CARGO_TARGET_DIR");

    let mut cmd = std::process::Command::new(cargo_cmd);
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_BUILD_RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
    cmd.env("CARGO_TARGET_DIR", target_dir);
    cmd.env("HC_DEMO_CLI_INCEPTION", "1");
    cmd.env("RUSTFLAGS", "-C opt-level=z");
    cmd.arg("build");
    cmd.arg("--release");
    cmd.arg("--lib");
    cmd.arg("--target").arg("wasm32-unknown-unknown");
    cmd.arg("--no-default-features");
    cmd.arg("--features").arg(tgt);

    println!("cargo:warning=HC_DEMO_CLI execute command: {cmd:?}");

    assert!(cmd.status().expect("build error").success(), "build error");

    let mut wasm = std::path::PathBuf::from(env!("CARGO_TARGET_DIR"));
    wasm.push("wasm32-unknown-unknown");
    wasm.push("release");
    wasm.push("hc_demo_cli.wasm");

    wasm
}
