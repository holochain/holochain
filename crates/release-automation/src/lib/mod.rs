#![allow(unused_imports)]
#![allow(dead_code)]

use anyhow::bail;
use anyhow::Context;
use comrak::{format_commonmark, parse_document, Arena, ComrakOptions};
use enumflags2::{bitflags, BitFlags};
use log::{debug, error, info, trace, warn};
use std::collections::{BTreeSet, HashSet};
use structopt::StructOpt;

use crate_selection::{aliases::CargoDepKind, CrateState, CrateStateFlags};
use release::ReleaseSteps;

pub mod changelog;
pub mod check;
pub mod common;
pub mod crate_;
pub mod crate_selection;
pub mod release;

#[cfg(test)]
pub mod tests;

pub type Fallible<T> = anyhow::Result<T>;
pub type CommandResult = Fallible<()>;

pub mod cli {
    use crate::common::SemverIncrementMode;
    use crate::crate_::CrateArgs;

    use super::*;
    use crate_selection::SelectionCriteria;
    use semver::Version;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[derive(Debug, StructOpt)]
    #[structopt(name = "release-automation")]
    pub struct Args {
        #[structopt(long)]
        pub workspace_path: PathBuf,

        #[structopt(subcommand)]
        pub cmd: Commands,

        #[structopt(long, default_value = "warn")]
        pub log_level: log::Level,

        #[structopt(long, default_value = "")]
        pub log_filters: String,

        /// Allows filtering to a subset of crates that will be processed for the given command.
        /// This string will be used as a regex to filter the package names.
        /// By default, all crates will be considered.
        #[structopt(long, default_value = ".*")]
        pub match_filter: fancy_regex::Regex,
    }

    #[derive(Debug, StructOpt)]
    #[structopt(name = "ra")]
    pub enum Commands {
        Changelog(ChangelogArgs),
        Release(ReleaseArgs),
        Check(CheckArgs),
        Crate(CrateArgs),
    }

    #[derive(Debug, StructOpt)]
    pub struct ChangelogAggregateArgs {
        /// Output path, relative to the workspace root.
        #[structopt(long, default_value = "CHANGELOG.md")]
        pub output_path: PathBuf,
    }

    #[derive(Debug, StructOpt)]
    pub struct ChangelogSetFrontmatterArgs {
        /// Activate dry-run mode which avoid changing any files
        #[structopt(long)]
        pub dry_run: bool,

        /// YAML file that defines the new frontmatter content. (will be validated by parsing)
        pub frontmatter_yaml_path: PathBuf,
    }

    #[derive(Debug, StructOpt)]
    pub enum ChangelogCommands {
        Aggregate(ChangelogAggregateArgs),
        SetFrontmatter(ChangelogSetFrontmatterArgs),
    }

    #[derive(StructOpt, Debug)]
    pub struct ChangelogArgs {
        #[structopt(subcommand)]
        pub command: ChangelogCommands,
    }

    /// Determine whether there are any release blockers by analyzing the state of the workspace.
    #[derive(StructOpt, Debug)]
    pub struct CheckArgs {
        /// All existing versions must match these requirements.
        /// Can be passed more than once to specify multiple.
        /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements
        #[structopt(long)]
        pub enforced_version_reqs: Vec<semver::VersionReq>,

        /// None of the existing versions are allowed to match these requirements.
        /// Can be passed more than once to specify multiple.
        /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements
        #[structopt(long)]
        pub disallowed_version_reqs: Vec<semver::VersionReq>,

        /// Allow these blocking states for dev dependency crates.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,

        /// Allow these blocking states for crates via the packages filter.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub allowed_matched_blockers: BitFlags<CrateStateFlags>,

        /// Exclude optional dependencies.
        #[structopt(long)]
        pub exclude_optional_deps: bool,

        /// If given, only these SemverIncrementMode values will be allowed.
        /// Comma separated.
        /// The values shall be given as YAML strings, matching the way they are configured in the CHANGELOG.md front matters.
        /// For a list of values and examples please see [this document](https://github.com/holochain/holochain/blob/develop/docs/release/release.md#permanently-marking-a-crate-for-majorminorpatchpre-version-bumps).
        #[structopt(long, parse(try_from_str = parse_semverincrementmode))]
        allowed_semver_increment_modes: Option<HashSet<SemverIncrementMode>>,
    }

    fn parse_depkind(input: &str) -> Fallible<HashSet<CargoDepKind>> {
        let mut set = HashSet::new();

        for word in input.split(',') {
            set.insert(match word.to_lowercase().as_str() {
                "" => continue,
                "normal" => CargoDepKind::Normal,
                "development" => CargoDepKind::Development,
                "build" => CargoDepKind::Build,

                invalid => bail!("invalid dependency kind: {}", invalid),
            });
        }

        Ok(set)
    }

    fn parse_cratestateflags(input: &str) -> Fallible<BitFlags<CrateStateFlags>> {
        use std::str::FromStr;

        input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|csf| {
                CrateStateFlags::from_str(csf)
                    .map_err(|_| anyhow::anyhow!("could not parse '{}' as CrateStateFlags", input))
            })
            .try_fold(
                Default::default(),
                |mut acc, elem| -> Fallible<BitFlags<_>> {
                    acc.insert(elem?);
                    Ok(acc)
                },
            )
    }

    fn parse_semverincrementmode(input: &str) -> Fallible<HashSet<SemverIncrementMode>> {
        input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|sim| {
                serde_yaml::from_str(sim).map_err(|_| {
                    anyhow::anyhow!("could not parse '{}' as SemverIncrementMode", input)
                })
            })
            .try_fold(
                Default::default(),
                |mut acc, elem| -> Fallible<HashSet<SemverIncrementMode>> {
                    acc.insert(elem?);
                    Ok(acc)
                },
            )
    }

    impl CheckArgs {
        /// Boilerplate to instantiate `SelectionCriteria` from `CheckArgs`
        pub fn to_selection_criteria(&self, args: &Args) -> SelectionCriteria {
            SelectionCriteria {
                match_filter: args.match_filter.clone(),
                disallowed_version_reqs: self.disallowed_version_reqs.clone(),
                enforced_version_reqs: self.enforced_version_reqs.clone(),
                allowed_dev_dependency_blockers: self.allowed_dev_dependency_blockers,
                allowed_selection_blockers: self.allowed_matched_blockers,
                exclude_optional_deps: self.exclude_optional_deps,
                allowed_semver_increment_modes: self.allowed_semver_increment_modes.clone(),
            }
        }
    }

    /// Initiate a release process with the given arguments.
    ///
    /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements for details on the requirements arguments.
    #[derive(StructOpt, Debug)]
    pub struct ReleaseArgs {
        #[structopt(flatten)]
        pub check_args: CheckArgs,

        #[structopt(long)]
        pub dry_run: bool,

        /// Will be inferred from the current name if not given.
        #[structopt(long)]
        pub release_branch_name: Option<String>,

        /// The release steps to perform.
        /// These will be reordered to their defined ordering.
        ///
        /// See `ReleaseSteps` for the list of steps.
        #[structopt(long, default_value="", parse(try_from_str = parse_releasesteps))]
        pub steps: BTreeSet<ReleaseSteps>,

        /// Force creation of the branch regardless of source branch.
        #[structopt(long)]
        pub force_branch_creation: bool,

        /// Force creation of the git tags.
        #[structopt(long)]
        pub force_tag_creation: bool,

        /// Force creation of the git tags.
        #[structopt(long)]
        pub no_tag_creation: bool,

        /// The dependencies that are allowed to be missing at the search location despite not being released.
        #[structopt(long, default_value="", parse(from_str = parse_string_set))]
        pub allowed_missing_dependencies: HashSet<String>,

        /// Set a custom CARGO_TARGET_DIR when shelling out to `cargo`.
        /// Currently only used for `cargo publish`.
        #[structopt(long)]
        pub cargo_target_dir: Option<PathBuf>,

        /// Don't run consistency verification checks.
        #[structopt(long)]
        pub no_verify: bool,

        /// Don't run consistency verification pre-change.
        #[structopt(long)]
        pub no_verify_pre: bool,

        /// Don't run consistency verification post-change.
        #[structopt(long)]
        pub no_verify_post: bool,

        /// Paths to manifest that will also be considered when updating the Cargo.lock files
        #[structopt(long)]
        pub additional_manifests: Vec<String>,

        #[structopt(
            long,
            default_value = crate_::MINIMUM_CRATE_OWNERS,
            use_delimiter = true,
            multiple = false,
        )]
        pub minimum_crate_owners: Vec<String>,
    }

    /// Parses a commad separated input string to a set of strings.
    pub fn parse_string_set(input: &str) -> HashSet<String> {
        use std::str::FromStr;

        input.split(',').filter(|s| !s.is_empty()).fold(
            Default::default(),
            |mut acc, elem| -> HashSet<_> {
                acc.insert(elem.to_string());
                acc
            },
        )
    }

    /// Parses an input string to an ordered set of release steps.
    pub fn parse_releasesteps(input: &str) -> Fallible<BTreeSet<ReleaseSteps>> {
        use std::str::FromStr;

        input
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|csf| {
                ReleaseSteps::from_str(csf).map_err(|_| {
                    anyhow::anyhow!("could not parse '{}' in '{}' as ReleaseSteps:", csf, input)
                })
            })
            .try_fold(
                Default::default(),
                |mut acc, elem| -> Fallible<BTreeSet<_>> {
                    acc.insert(elem?);
                    Ok(acc)
                },
            )
    }
}
