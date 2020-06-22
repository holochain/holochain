use std::path::Path;

fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=*");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
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

    for &m in [
        "anchor",
        "bench",
        "debug",
        "entry_defs",
        "foo",
        "imports",
        "init_pass",
        "init_fail",
        "migrate_agent_pass",
        "migrate_agent_fail",
        "post_commit_success",
        "post_commit_fail",
        "validate",
        "validate_invalid",
        "validate_valid",
        "validation_package_fail",
        "validation_package_success",
    ]
    .iter()
    {
        let cargo_toml = Path::new(m).join("Cargo.toml");

        let cargo_command = std::env::var_os("CARGO");
        let cargo_command = cargo_command.as_deref().unwrap_or_else(|| "cargo".as_ref());

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
