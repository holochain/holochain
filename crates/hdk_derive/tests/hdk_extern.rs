use std::fs;
use std::path::PathBuf;

fn find_deps_dir() -> PathBuf {
    let mut current_dir = std::env::current_dir().unwrap();

    while !current_dir.join("target/debug/deps").exists() {
        if !current_dir.pop() {
            panic!("Could not find target/debug/deps directory");
        }
    }

    current_dir.join("target/debug/deps")
}

fn run_mode(mode: &'static str) {
    let deps_path = find_deps_dir();
    println!("Found deps directory at: {}", deps_path.display());

    // Find library files
    let hdk_lib = fs::read_dir(&deps_path)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().starts_with("libhdk-"))
        .expect("Could not find hdk library");

    let hdk_derive_lib = fs::read_dir(&deps_path)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("libhdk_derive-")
        })
        .expect("Could not find hdk_derive library");

    let target_rustcflags = format!(
        "-L dependency={} \
         --extern hdk={} \
         --extern hdk_derive={} \
         --edition 2021",
        deps_path.display(),
        hdk_lib.path().display(),
        hdk_derive_lib.path().display()
    );

    let config = compiletest_rs::Config {
        mode: mode.parse().expect("Invalid mode"),
        src_base: PathBuf::from("tests/fail"),
        target_rustcflags: Some(target_rustcflags),
        ..Default::default()
    };

    compiletest_rs::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("compile-fail");
}
