use std::{
    io::{Read, Write},
    path::Path,
};

use semver::VersionReq;

use super::*;

pub(crate) fn selection_check<'a>(
    cmd_args: &'a crate::cli::CheckArgs,
    ws: &'a crate::crate_selection::ReleaseWorkspace<'a>,
) -> Fallible<Vec<&'a crate_selection::Crate<'a>>> {
    debug!("cmd_args: {:#?}", cmd_args);

    let release_selection = ws.release_selection()?;

    info!(
        "crates selected for the release process: {:#?}",
        release_selection
            .iter()
            .map(|crt| format!("{}-{}", crt.name(), crt.version()))
            .collect::<Vec<_>>()
    );

    Ok(release_selection)
}

/// Sets the new version for the given crate and returns the dependants of the crates for post-processing them.
pub(crate) fn set_version<'a>(
    dry_run: bool,
    crt: &'a crate_selection::Crate<'a>,
    release_version: &semver::Version,
) -> Fallible<Vec<&'a crate_selection::Crate<'a>>> {
    let cargo_toml_path = crt.root().join("Cargo.toml");
    debug!(
        "setting version to {} in manifest at {:?}",
        release_version, cargo_toml_path
    );
    if !dry_run {
        cargo_next::set_version(&cargo_toml_path, release_version.to_string())?;
    }

    let dependants = crt
        .dependants_in_workspace_filtered(|(_dep_name, deps)| {
            deps.iter()
                .any(|dep| dep.version_req() != &cargo::util::OptVersionReq::from(VersionReq::STAR))
        })?
        .to_owned();

    for dependant in dependants.iter() {
        let target_manifest = dependant.manifest_path();

        debug!(
            "[{}] updating dependency version from dependant {} to version {} in manifest {:?}",
            crt.name(),
            dependant.name(),
            release_version.to_string().as_str(),
            &target_manifest,
        );

        if !dry_run {
            set_dependency_version(
                target_manifest,
                &crt.name(),
                release_version.to_string().as_str(),
            )?;
        }
    }

    Ok(dependants)
}

// Adapted from https://github.com/sunng87/cargo-release/blob/f94938c3f20ef20bc8f971d59de75574a0b18931/src/cargo.rs#L122-L154
fn set_dependency_version(manifest_path: &Path, name: &str, version: &str) -> Fallible<()> {
    let temp_manifest_path = manifest_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("couldn't get parent of path {}", manifest_path.display()))?
        .join("Cargo.toml.work");

    {
        let manifest = load_from_file(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse()?;
        for key in &["dependencies", "dev-dependencies", "build-dependencies"] {
            if manifest.as_table().contains_key(key)
                && manifest[key]
                    .as_table()
                    .expect("manifest is already verified")
                    .contains_key(name)
            {
                let existing_version = manifest[key][name]["version"].as_str().unwrap_or("*");

                if *key == "dependencies" || !existing_version.contains("*") {
                    manifest[key][name]["version"] = toml_edit::value(version);
                }
            }
        }

        let mut file_out = std::fs::File::create(&temp_manifest_path)?;
        file_out.write_all(manifest.to_string_in_original_order().as_bytes())?;
    }
    std::fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

#[cfg(test)]
pub(crate) fn get_dependency_version(manifest_path: &Path, name: &str) -> Fallible<String> {
    let manifest_path = manifest_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("couldn't get parent of path {}", manifest_path.display()))?
        .join("Cargo.toml");

    {
        let manifest: toml_edit::Document = load_from_file(&manifest_path)?.parse()?;
        for key in &["dependencies", "dev-dependencies", "build-dependencies"] {
            if manifest.as_table().contains_key(key)
                && manifest[key]
                    .as_table()
                    .expect("manifest is already verified")
                    .contains_key(name)
            {
                return Ok(manifest[key][name]["version"]
                    .as_value()
                    .ok_or_else(|| anyhow::anyhow!("expected a value"))?
                    .to_string());
            }
        }
    }

    bail!("version not found")
}

fn load_from_file(path: &Path) -> Fallible<String> {
    let mut file = std::fs::File::open(path)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}
