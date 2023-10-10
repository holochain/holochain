#[cfg(not(build_wasm))]
fn main() {}

#[cfg(build_wasm)]
fn main() {
    println!("cargo:rerun-if-env-changed=HC_DEMO_CLI_INCEPTION");

    if std::env::var_os("HC_DEMO_CLI_INCEPTION").is_some() {
        return;
    }

    let cargo_cmd = std::env::var_os("CARGO");
    let cargo_cmd = cargo_cmd.as_deref().unwrap_or_else(|| "cargo".as_ref());

    build(cargo_cmd, "integrity");
    build(cargo_cmd, "coordinator");
}

#[cfg(build_wasm)]
fn find_target_dir() -> std::path::PathBuf {
    let mut target_dir =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("failed getting out_dir"));
    target_dir.pop(); // "out"
    target_dir.pop(); // "crate-[hash]"
    target_dir.pop(); // "build"
    target_dir.pop(); // "debug"
    println!("cargo:warning=TARGET_DIR: {target_dir:?}");
    target_dir
}

#[cfg(build_wasm)]
fn build(cargo_cmd: &std::ffi::OsStr, tgt: &str) {
    let target_dir = find_target_dir();
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();

    let mut cmd = std::process::Command::new(cargo_cmd);
    cmd.env_remove("RUSTFLAGS");
    cmd.env_remove("CARGO_BUILD_RUSTFLAGS");
    cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
    cmd.env("CARGO_TARGET_DIR", target_dir.clone());
    cmd.env("HC_DEMO_CLI_INCEPTION", "1");
    cmd.env("RUSTFLAGS", "-C opt-level=z");
    cmd.arg("build");
    cmd.arg("--release");
    cmd.arg("--lib");
    cmd.arg("--target").arg("wasm32-unknown-unknown");
    cmd.arg("--no-default-features");
    cmd.arg("--features").arg(format!("build_{tgt}_wasm"));

    println!("cargo:warning=HC_DEMO_CLI execute command: {cmd:?}");

    assert!(cmd.status().expect("build error").success(), "build error");

    let mut wasm = std::path::PathBuf::from(&target_dir);
    wasm.push("wasm32-unknown-unknown");
    wasm.push("release");
    wasm.push("hc_demo_cli.wasm");

    let mut opt_wasm = std::path::PathBuf::from(&target_dir);
    opt_wasm.push("wasm32-unknown-unknown");
    opt_wasm.push("release");
    opt_wasm.push(format!("{tgt}.opt.wasm"));

    println!("cargo:warning=HC_DEMO_CLI opt wasm: from: {wasm:?}, to: {opt_wasm:?}");
    wasm_opt::OptimizationOptions::new_optimize_for_size()
        .run(wasm, &opt_wasm)
        .unwrap();

    let mut to = std::path::PathBuf::from(manifest_dir);
    to.push("src");
    to.push(format!("{tgt}.wasm.gz"));

    println!("cargo:warning=HC_DEMO_CLI gz wasm: from: {opt_wasm:?}, to: {to:?}");

    let opt_wasm = std::fs::read(&opt_wasm).unwrap();

    let to = std::fs::File::create(to).unwrap();
    let mut gz = flate2::GzBuilder::new().write(to, flate2::Compression::best());
    std::io::Write::write_all(&mut gz, &opt_wasm).unwrap();
    gz.finish().unwrap();
}
