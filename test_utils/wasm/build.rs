use std::path::Path;

fn main() {
    // force a rebuild every time because:
    // - the wasm .rs files changing doesn't trigger a rebuild here
    // - any shared dependency with core can be changed and that won't trigger a rebuild
    // the alternative is to recurse over all the files in the dependencies and wasm
    // crates and rerun-if-changed on each of those
    // if you want to implement that, go for it :)
    println!("cargo:rerun-if-changed=file-not-exists-forces-rebuild");

    for m in vec!["debug", "foo", "imports"] {

        let out_dir = std::env::var_os("OUT_DIR").unwrap();
        let toml_str = format!("{}/Cargo.toml", &m);
        let cargo_toml = Path::new(&toml_str);

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
            .env("CARGO_TARGET_DIR", out_dir)
            .status()
            .unwrap();

        assert!(status.success());

    }
}
