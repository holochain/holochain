use std::{
    io::{Read, Write},
    path::Path,
};

use semver::VersionReq;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SemverIncrementMode {
    Major,
    Minor,
    Patch,
}

impl Default for SemverIncrementMode {
    fn default() -> Self {
        Self::Patch
    }
}

/// Increment the given Version according to the given `SemVerIncrementMode`.
pub(crate) fn increment_semver(v: &mut semver::Version, mode: SemverIncrementMode) {
    match mode {
        SemverIncrementMode::Major => {
            v.major += 1;
            v.minor = 0;
            v.patch = 0;
        }
        SemverIncrementMode::Minor => {
            v.minor += 1;
            v.patch = 0;
        }
        SemverIncrementMode::Patch => {
            v.patch += 1;
        }
    }

    v.pre = semver::Prerelease::EMPTY;
    v.build = semver::BuildMetadata::EMPTY;
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use crate::common::{
        increment_semver,
        SemverIncrementMode::{self, *},
    };

    #[test_case("0.0.1", "0.0.2", Patch; "patch version bump")]
    #[test_case("0.0.1", "0.1.0", Minor; "minor version bump")]
    #[test_case("0.0.1", "1.0.0", Major; "major version bump")]
    #[test_case("0.0.1-dev.0", "0.0.2", Patch; "patch version bump from pre-release")]
    #[test_case("0.0.1-dev.0", "0.1.0", Minor; "minor version bump from pre-release")]
    #[test_case("0.0.1-dev.0", "1.0.0", Major; "major version bump from pre-release")]
    #[test_case("0.1.1-dev.0", "0.2.0", Minor; "non-zero minor version bump from pre-release")]
    #[test_case("1.0.1-dev.0", "2.0.0", Major; "non-zero major version bump from pre-release")]
    fn increment_semver_consistency(
        input_version: &str,
        expected_version: &str,
        increment_mode: SemverIncrementMode,
    ) {
        let mut working_version = semver::Version::parse(input_version).unwrap();
        increment_semver(&mut working_version, increment_mode);

        let expected_version = semver::Version::parse(expected_version).unwrap();
        assert_eq!(expected_version, working_version);
    }
}
