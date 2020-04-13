use std::path::Path;

fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();

    // force a rebuild every time because:
    // - the wasm .rs files changing doesn't trigger a rebuild here
    // - any shared dependency with core can be changed and that won't trigger a rebuild
    // the alternative is to recurse over all the files in the dependencies and wasm
    // crates and rerun-if-changed on each of those
    // if you want to implement that, go for it :)
    let hacky_file_name = "__wasm_test_utils_non-existent_file";
    let hacky_file_not_found = match std::fs::metadata(hacky_file_name) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        _ => false,
    };
    assert!(
        hacky_file_not_found,
        "hack: {} must not exist in package directory for build to continue",
        hacky_file_name
    );

    println!("cargo:rerun-if-changed={}", hacky_file_name);

    for &m in ["debug", "foo", "imports"].iter() {
        let cargo_toml = Path::new(m).join("Cargo.toml");

        let cargo_command = std::env::var_os("CARGO");
        let cargo_command = cargo_command
            .as_ref()
            .map(|s| &**s)
            .unwrap_or_else(|| "cargo".as_ref());

        let status = std::process::Command::new(cargo_command)
            .arg("build")
            .arg("--manifest-path")
            .arg(cargo_toml)
            .arg("--release")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .env("CARGO_TARGET_DIR", &out_dir)
            .status()
            .unwrap();

        assert!(status.success());

    }
}
