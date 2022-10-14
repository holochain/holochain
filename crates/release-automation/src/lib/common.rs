use std::{
    io::{Read, Write},
    path::Path,
};

use cargo::util::VersionExt;
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
#[serde(rename_all = "snake_case")]
pub(crate) enum SemverIncrementMode {
    Major,
    Minor,
    Patch,
    Pre(String),
    PreMajor(String),
    PreMinor(String),
    PrePatch(String),
}

impl Default for SemverIncrementMode {
    fn default() -> Self {
        Self::Patch
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub(crate) enum SemverIncrementError {
    #[error("resulting version ({result}) is lower than on entry ({entry})")]
    ResultingVersionLower {
        result: semver::Version,
        entry: semver::Version,
    },

    #[error("pre-release increment requested but none found on entry ({entry})")]
    MissingPreRelease { entry: semver::Version },
}

fn evaluate_suffix<'a>(suffix_requested: &'a str, pre: &'a semver::Prerelease) -> (&'a str, usize) {
    let (suffix, counter) = match pre.rsplit_once('.') {
        Some((suffix, maybe_number)) => {
            if let Ok(number) = maybe_number.parse::<usize>() {
                (suffix, number + 1)
            } else {
                (pre.as_str(), 0)
            }
        }

        None => (pre.as_str(), 0),
    };

    if suffix != suffix_requested {
        (suffix_requested, 0)
    } else {
        (suffix, counter)
    }
}

/// Implements version incrementation as specified in [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html)
/// Currently resets [Build metadata](https://semver.org/spec/v2.0.0.html#spec-item-10).
pub(crate) fn increment_semver<'a>(
    v: &'a mut semver::Version,
    mode: SemverIncrementMode,
) -> Fallible<()> {
    use SemverIncrementMode::*;

    v.build = semver::BuildMetadata::EMPTY;

    let entry_version = v.clone();

    match mode {
        Major => {
            if !v.pre.is_empty() && v.patch == 0 && v.minor == 0 {
                v.pre = semver::Prerelease::EMPTY;
            } else {
                v.major += 1;
                v.minor = 0;
                v.patch = 0;
                v.pre = semver::Prerelease::EMPTY;
            }
        }
        Minor => {
            if !v.pre.is_empty() && v.patch == 0 {
                v.pre = semver::Prerelease::EMPTY;
            } else {
                v.minor += 1;
                v.patch = 0;
                v.pre = semver::Prerelease::EMPTY;
            }
        }
        Patch => {
            if !v.pre.is_empty() {
                v.pre = semver::Prerelease::EMPTY;
            } else {
                v.patch += 1;
                v.pre = semver::Prerelease::EMPTY;
            }
        }

        pre_modes => {
            let (suffix, counter) = match &pre_modes {
                Major | Minor | Patch => unreachable!(
                    r"
                        this arm is already fully covered in the surrounding match statement.
                        i'm surprised the compiler doesn't understand this ¯\_(ツ)_/¯
                    "
                ),

                Pre(suffix_requested) => {
                    if !v.is_prerelease() {
                        bail!(SemverIncrementError::MissingPreRelease { entry: v.clone() });
                    }

                    evaluate_suffix(&suffix_requested, &v.pre)
                }

                PreMajor(suffix_requested) => {
                    if v.minor == 0 && v.patch == 0 && v.is_prerelease() {
                        evaluate_suffix(suffix_requested.as_str(), &v.pre)
                    } else {
                        increment_semver(v, SemverIncrementMode::Major)?;
                        (suffix_requested.as_str(), 0)
                    }
                }

                PreMinor(suffix_requested) => {
                    if v.patch == 0 && v.is_prerelease() {
                        evaluate_suffix(suffix_requested.as_str(), &v.pre)
                    } else {
                        increment_semver(v, SemverIncrementMode::Minor)?;
                        (suffix_requested.as_str(), 0)
                    }
                }

                PrePatch(suffix_requested) => {
                    if v.is_prerelease() {
                        evaluate_suffix(suffix_requested.as_str(), &v.pre)
                    } else {
                        increment_semver(v, SemverIncrementMode::Patch)?;
                        (suffix_requested.as_str(), 0)
                    }
                }
            };

            let final_pre = format!("{}.{}", suffix, counter);
            v.pre = semver::Prerelease::new(&final_pre)?;
        }
    }

    if &entry_version >= &v {
        bail!(SemverIncrementError::ResultingVersionLower {
            result: v.clone(),
            entry: entry_version,
        });
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use crate::common::{
        increment_semver,
        SemverIncrementMode::{self, *},
    };

    use super::SemverIncrementError;

    //
    // Major
    //
    #[test_case(Major, "0.0.0-dev.0", "0.0.0")]
    #[test_case(Major, "0.0.1-dev.0", "1.0.0")]
    #[test_case(Major, "0.0.1", "1.0.0")]
    #[test_case(Major, "1.0.0-dev.0", "1.0.0")]
    #[test_case(Major, "1.0.0", "2.0.0")]
    #[test_case(Major, "1.0.1-dev.0", "2.0.0")]
    #[test_case(Major, "1.1.1-dev.0", "2.0.0")]
    #[test_case(Major, "1.1.1", "2.0.0")]
    //
    // Minor
    //
    #[test_case(Minor, "0.0.1", "0.1.0")]
    #[test_case(Minor, "0.0.1-dev.0", "0.1.0")]
    #[test_case(Minor, "0.1.1-dev.0", "0.2.0")]
    //
    // Patch
    //
    #[test_case(Patch, "0.0.0-dev.0", "0.0.0")]
    #[test_case(Patch, "0.0.1-dev.0", "0.0.1")]
    #[test_case(Patch, "0.0.1", "0.0.2")]
    #[test_case(Patch, "0.1.1", "0.1.2")]
    #[test_case(Patch, "1.1.1", "1.1.2")]
    //
    // Pre
    //
    #[test_case(Pre("rc".to_string()), "0.0.1-dev.0", "0.0.1-rc.0")]
    #[test_case(Pre("rc".to_string()), "0.0.1-rc.0", "0.0.1-rc.1")]
    //
    // PreMajor
    //
    #[test_case(PreMajor("rc".to_string()), "0.0.0", "1.0.0-rc.0")]
    #[test_case(PreMajor("rc".to_string()), "0.0.1", "1.0.0-rc.0")]
    #[test_case(PreMajor("rc".to_string()), "0.1.0", "1.0.0-rc.0")]
    #[test_case(PreMajor("rc".to_string()), "0.1.1", "1.0.0-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PreMajor("rc".to_string()), "1.0.0-rc", "1.0.0-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PreMajor("rc".to_string()), "1.0.0-rc.0", "1.0.0-rc.1")]
    //
    // PreMinor
    //
    #[test_case(PreMinor("rc".to_string()), "0.0.0", "0.1.0-rc.0")]
    #[test_case(PreMinor("rc".to_string()), "0.0.1-rc.0", "0.1.0-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PreMinor("rc".to_string()), "0.1.0-rc", "0.1.0-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PreMinor("rc".to_string()), "0.1.0-rc.0", "0.1.0-rc.1")]
    //
    // PrePatch
    //
    #[test_case(PrePatch("rc".to_string()), "0.0.0", "0.0.1-rc.0")]
    #[test_case(PrePatch("rc".to_string()), "0.0.1", "0.0.2-rc.0")]
    #[test_case(PrePatch("rc".to_string()), "0.1.1", "0.1.2-rc.0")]
    #[test_case(PrePatch("rc".to_string()), "1.1.0", "1.1.1-rc.0")]
    #[test_case(PrePatch("rc".to_string()), "1.1.1-rc", "1.1.1-rc.0")]
    #[test_case(PrePatch("rc".to_string()), "1.1.1-rc.0", "1.1.1-rc.1")]
    #[test_case(PrePatch("rc".to_string()), "1.0.0", "1.0.1-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PrePatch("rc".to_string()), "0.0.0-rc", "0.0.0-rc.0")]
    // TODO: check with someone else if this seems counter-intuitive
    #[test_case(PrePatch("rc".to_string()), "0.0.0-rc.0", "0.0.0-rc.1")]
    fn increment_semver_consistency(
        increment_mode: SemverIncrementMode,
        input_version: &str,
        expected_version: &str,
    ) {
        let mut working_version = semver::Version::parse(input_version).unwrap();
        increment_semver(&mut working_version, increment_mode).unwrap();

        let expected_version = semver::Version::parse(expected_version).unwrap();
        assert_eq!(expected_version, working_version);
    }

    //
    // errors
    //
    #[test_case(Pre("rc".to_string()), "0.0.1", SemverIncrementError::MissingPreRelease { entry:  semver::Version::new(0,0,1)})]
    #[test_case(Pre("a".to_string()), "0.0.1-b", SemverIncrementError::ResultingVersionLower { result: semver::Version::parse("0.0.1-a.0").unwrap(), entry:  semver::Version::parse("0.0.1-b").unwrap()})]
    fn increment_semver_consistency_failure(
        increment_mode: SemverIncrementMode,
        input_version: &str,
        expected_error: SemverIncrementError,
    ) {
        let mut working_version = semver::Version::parse(input_version).unwrap();
        let err: SemverIncrementError = increment_semver(&mut working_version, increment_mode)
            .unwrap_err()
            .downcast()
            .unwrap();

        assert_eq!(expected_error, err, "{:?}", err)
    }
}
