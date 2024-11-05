use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

fn main() {
    let should_build = std::env::var_os("CARGO_FEATURE_BUILD").is_some();
    let only_check = std::env::var_os("CARGO_FEATURE_ONLY_CHECK").is_some();
    let enable_unstable_functions = std::env::var_os("CARGO_FEATURE_UNSTABLE_FUNCTIONS").is_some();

    if !(should_build || only_check) {
        return;
    }

    let wasms_path = format!("{}/{}/", env!("CARGO_MANIFEST_DIR"), "wasm_workspace");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");

    // We want to rebuild if anything upstream of the wasms has changed.
    // Since we use local paths, changes to those crates will not affect the
    // Cargo.toml, so we check each upstream local source directory directly.
    for dir in parse_cargo_toml_local_dependency_paths() {
        for item in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            println!("cargo:rerun-if-changed={}", item.path().display());
        }
    }
    // If any of the files in the wasms change rebuild
    for item in walkdir::WalkDir::new(wasms_path.clone())
        .into_iter()
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|e| e != "target")
                .unwrap_or(false)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        println!("cargo:rerun-if-changed={}", item.path().display());
    }
    let wasm_out = std::env::var_os("HC_TEST_WASM_DIR");
    let cargo_command = std::env::var_os("CARGO");
    let cargo_command = cargo_command.as_deref().unwrap_or_else(|| "cargo".as_ref());

    build_test_wasms(
        &wasm_out,
        cargo_command,
        should_build,
        false,
        enable_unstable_functions,
        &wasms_path,
    );
    build_test_wasms(
        &wasm_out,
        cargo_command,
        should_build,
        true,
        enable_unstable_functions,
        &wasms_path,
    );
}

fn build_test_wasms(
    wasm_out: &Option<std::ffi::OsString>,
    cargo_command: &std::ffi::OsStr,
    should_build: bool,
    build_integrity_zomes: bool,
    enable_unstable_functions: bool,
    wasms_path: &str,
) {
    let paths = list_wasms(PathBuf::from(wasms_path));
    for path in paths {
        let project = load_project_toml(path.clone());
        let mut cmd = std::process::Command::new(cargo_command);
        cmd.env_remove("RUSTFLAGS");
        cmd.env_remove("CARGO_BUILD_RUSTFLAGS");
        cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
        if should_build {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd.env("RUSTFLAGS", "-C opt-level=z");
            cmd.arg("build")
                .arg("--manifest-path")
                .arg(&path)
                .arg("--release")
                .arg("--target")
                .arg("wasm32-unknown-unknown");

            if enable_unstable_functions && defines_feature(&project, "unstable-functions") {
                cmd.arg("--features").arg("unstable-functions");
            }
        } else {
            cmd.arg("check")
                .arg("--manifest-path")
                .arg(&path);
        }
        if build_integrity_zomes {
            let mut features = "".to_string();
            if defines_feature(&project, "integrity") {
                features.push_str("integrity");
            }
            if enable_unstable_functions && defines_feature(&project, "unstable-functions") {
                if !features.is_empty() {
                    features.push_str(",");
                }
                features.push_str("unstable-functions");
            }

            cmd.arg("--examples");
            cmd.arg("--no-default-features");

            if !features.is_empty() {
                cmd.arg("--features");
                cmd.arg(features);
            }
        }
        match wasm_out {
            Some(wasm_out) => {
                cmd.env("CARGO_TARGET_DIR", wasm_out);
            }
            None => {
                cmd.env("CARGO_TARGET_DIR", format!("{}/target", wasms_path));
            }
        }
        let output = cmd.output().unwrap();
        if !output.status.success() {
            std::io::stderr().write_all(&output.stderr).ok();
            eprintln!("While building {:?}", path);
            assert!(output.status.success());
        }
    }
}

/// Return the list of local path dependencies specified in the Cargo.toml
fn parse_cargo_toml_local_dependency_paths() -> Vec<String> {
    let cargo_toml: toml::Value = std::fs::read_to_string("Cargo.toml")
        .unwrap()
        .parse()
        .unwrap();
    let mut table = toml_table(cargo_toml);

    let deps: Vec<_> = match (
        table.remove("dependencies"),
        table.remove("dev-dependencies"),
    ) {
        (Some(deps), Some(dev_deps)) => toml_table(deps)
            .values()
            .chain(toml_table(dev_deps).values())
            .cloned()
            .collect(),
        (Some(deps), None) => toml_table(deps).values().cloned().collect(),
        (None, Some(dev_deps)) => toml_table(dev_deps).values().cloned().collect(),
        (None, None) => Vec::new(),
    };

    deps.into_iter()
        .filter_map(|v| {
            if let toml::Value::Table(mut table) = v {
                table.remove("path").map(toml_string)
            } else {
                None
            }
        })
        .collect()
}

fn list_wasms(wasms_path: PathBuf) -> Vec<PathBuf> {
    let project = std::fs::read_to_string(wasms_path.join("Cargo.toml"))
        .expect("Could not find workspace Cargo.toml");
    let project = toml_table(
        toml::from_str::<toml::Value>(&project).expect("Could not parse workspace Cargo.toml"),
    );
    let workspace = toml_table(
        project
            .get("workspace")
            .expect("Could not find workspace in Cargo.toml")
            .clone(),
    );
    let members = toml_array(
        workspace
            .get("members")
            .expect("Could not find members in workspace")
            .clone(),
    );
    members
        .into_iter()
        .map(|v| {
            let member = toml_string(v);
            let path = wasms_path.join(member);
            path.join("Cargo.toml")
        })
        .collect()
}

fn load_project_toml(cargo_toml: PathBuf) -> toml::value::Table {
    let project = std::fs::read_to_string(cargo_toml)
        .expect("Could not load Cargo.toml");

    toml_table(
        toml::from_str::<toml::Value>(&project).expect("Could not parse Cargo.toml"),
    )
}

fn defines_feature(project: &toml::value::Table, feature: &str) -> bool {
    if let Some(features) = project.get("features") {
        let features = toml_table(features.clone());
        if features.contains_key(feature) {
            return true;
        }
    }

    false
}

/// Interpret toml Value as a String or panic
fn toml_string(value: toml::Value) -> String {
    if let toml::Value::String(string) = value {
        string
    } else {
        panic!("Expected TOML string, got: {:?}", value)
    }
}

/// Interpret toml Value as a Table or panic
fn toml_table(value: toml::Value) -> toml::value::Table {
    if let toml::Value::Table(table) = value {
        table
    } else {
        panic!("Expected TOML table, got: {:?}", value)
    }
}

/// Interpret toml Value as a Table or panic
fn toml_array(value: toml::Value) -> toml::value::Array {
    if let toml::Value::Array(array) = value {
        array
    } else {
        panic!("Expected TOML array, got: {:?}", value)
    }
}
