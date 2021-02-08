use std::io::Write;
use std::process::Stdio;

fn main() {
    let should_build = std::env::var_os("CARGO_FEATURE_BUILD").is_some();
    let only_check = std::env::var_os("CARGO_FEATURE_ONLY_CHECK").is_some();

    if !(should_build || only_check) {
        return;
    }

    let wasms_path = format!("{}/{}/", env!("CARGO_MANIFEST_DIR"), "wasm_workspace");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    println!("cargo:rerun-if-changed=*");
    // We want to rebuild if anything upstream of the wasms has changed.
    // Since we use local paths, changes to those crates will not affect the
    // Cargo.toml, so we check each upstream local source directory directly.
    for dir in parse_cargo_toml_local_dependency_paths() {
        println!("cargo:rerun-if-changed={}", dir);
        for item in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
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
    {
        println!("cargo:rerun-if-changed={}", item.path().display());
    }
    let wasm_out = std::env::var_os("HC_TEST_WASM_DIR");
    let cargo_command = std::env::var_os("CARGO");
    let cargo_command = cargo_command.as_deref().unwrap_or_else(|| "cargo".as_ref());
    if should_build {
        let mut cmd = std::process::Command::new(cargo_command);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.arg("build")
            .arg("--manifest-path")
            .arg("wasm_workspace/Cargo.toml")
            .arg("--release")
            .arg("--workspace")
            .arg("--target")
            .arg("wasm32-unknown-unknown");
        match wasm_out {
            Some(wasm_out) => {
                cmd.env("CARGO_TARGET_DIR", wasm_out);
            }
            None => {
                cmd.env("CARGO_TARGET_DIR", format!("{}/target", wasms_path));
            }
        }
        let output = cmd.output().unwrap();

        assert!(
            output.status.success(),
            std::io::stderr().write_all(&output.stderr)
        );
    } else {
        let mut cmd = std::process::Command::new(cargo_command);
        cmd.arg("check")
            .arg("--manifest-path")
            .arg("wasm_workspace/Cargo.toml");
        match wasm_out {
            Some(wasm_out) => {
                cmd.env("CARGO_TARGET_DIR", wasm_out);
            }
            None => {
                cmd.env("CARGO_TARGET_DIR", format!("{}/target", wasms_path));
            }
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            std::io::stderr().write_all(&output.stderr)
        );
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
