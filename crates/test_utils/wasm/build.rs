use std::path::Path;

fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();

    // // HACK(thedavidmeister): We force a rebuild of our included Wasm packages
    // // every time this crate is built.
    // //
    // // Without this hack, changes made to the Wasm's dependencies that live in
    // // this repo wouldn't always trigger a rebuild of the Wasm and we could end
    // // up in inconsistent and confusing states.
    // //
    // // TODO: Investigate options like only rebuilding if a file in `crates/`
    // // has changed.
    // //
    // // See also: https://github.com/rust-lang/cargo/issues/8091
    // let hacky_file_name = "__wasm_test_utils_non-existent_file";
    // let hacky_file_not_found = match std::fs::metadata(hacky_file_name) {
    //     Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
    //     _ => false,
    // };
    // assert!(
    //     hacky_file_not_found,
    //     "hack: {} must not exist in package directory for build to continue",
    //     hacky_file_name
    // );

    // println!("cargo:rerun-if-changed={}", hacky_file_name);

    // for &m in ["debug", "foo", "imports"].iter() {
    //     let cargo_toml = Path::new(m).join("Cargo.toml");

    //     let cargo_command = std::env::var_os("CARGO");
    //     let cargo_command = cargo_command
    //         .as_ref()
    //         .map(|s| &**s)
    //         .unwrap_or_else(|| "cargo".as_ref());

    //     let status = std::process::Command::new(cargo_command)
    //         .arg("build")
    //         .arg("--manifest-path")
    //         .arg(cargo_toml)
    //         .arg("--release")
    //         .arg("--target")
    //         .arg("wasm32-unknown-unknown")
    //         .env("CARGO_TARGET_DIR", &out_dir)
    //         .status()
    //         .unwrap();

    //     assert!(status.success());
    // }
}
