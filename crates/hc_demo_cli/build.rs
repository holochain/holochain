fn main() {
    let inception = std::env::var_os("HC_DEMO_CLI_INCEPTION").is_some();

    println!("cargo:warning=inception: {inception}");

    if inception {
        return;
    }

    let cargo_cmd = std::env::var_os("CARGO");
    let cargo_cmd = cargo_cmd.as_deref().unwrap_or_else(|| "cargo".as_ref());

    println!("cargo:warning=cargo cmd: {cargo_cmd:?}");

    let out_dir = std::env::var_os("OUT_DIR").expect("have out dir");

    println!("cargo:warning=out dir: {out_dir:?}");

    let mut integrity_out = std::path::PathBuf::from(&out_dir);
    integrity_out.push("integrity");

    println!("cargo:warning=integrity out: {integrity_out:?}");

    let mut coordinator_out = std::path::PathBuf::from(&out_dir);
    coordinator_out.push("coordinator");

    println!("cargo:warning=coordinator out: {coordinator_out:?}");

    build(cargo_cmd, "build_integrity_wasm", &integrity_out);
    build(cargo_cmd, "build_coordinator_wasm", &coordinator_out);
}

fn build(cargo_cmd: &std::ffi::OsStr, tgt: &str, out: &std::path::Path) {
    let mut cmd = std::process::Command::new(cargo_cmd);
    cmd.env("HC_DEMO_CLI_INCEPTION", "1");
    cmd.env("CARGO_NET_OFFLINE", "1");
    cmd.env("CARGO_TARGET_DIR", out);
    cmd.env("RUSTFLAGS", "-C opt-level=z");
    cmd.arg("build");
    cmd.arg("--offline");
    cmd.arg("--release");
    cmd.arg("--lib");
    cmd.arg("--target").arg("wasm32-unknown-unknown");
    cmd.arg("--no-default-features");
    cmd.arg("--features").arg(tgt);

    println!("cargo:warning=execute command: {cmd:?}");

    assert!(cmd.status().expect("build error").success(), "build error");
}
