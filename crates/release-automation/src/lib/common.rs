use std::{
    io::{Read, Write},
    path::Path,
};

use cargo::util::VersionExt;
use semver::{Comparator, VersionReq};
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

/// Load a file into a String
pub(crate) fn load_from_file(path: &Path) -> Fallible<String> {
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
    #[test_case(Minor, "0.1.0-beta-rc.1", "0.1.0")]
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
    #[test_case(PreMinor("beta-rc".to_string()), "0.0.170", "0.1.0-beta-rc.0")]
    #[test_case(PreMinor("beta-rc".to_string()), "0.1.0-beta-rc.0", "0.1.0-beta-rc.1")]
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
