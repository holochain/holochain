/*!
# Release Automation

This project codifies Holochain's opinionated release workflow.
It supports selectively releasing crates within a [cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) with flexible handling of release blockers.
The aim is to build a CLI tool that can be used manually and also within the context of CI for fully automated releases.

## Workflow

The workflow is split up into multiple steps that involve different branches.

Each release involves three branches:
- **develop**: this is where development takes place on a day to day bases.
- **release-YYYYMMDD.HHMMSS**: for each release _develop_ is branched off into a new release branch with this naming scheme.
- **main**: release branches are merged into this for downstream consumption.

### Brief summary

0. Decide it's the time for a new release
1. Create a new release branch from develop
2. For the main crates and all of there dependencies in the workspace:
    - Determine candidates by all of the positive indicators signaling:
        * they have changed since their last release by looking at their CHANGELOG.md OR they haven't had a release
        * version number is allowed by a the requirement
    - Skip candidates by any of these negative indicators signalling:
        * CHANGELOG.md contains `unreleaseable = true` in its front matter
        * version number is disallowed by a requirement
3. Increase the package version in each Cargo.toml file
4. Add a release in each CHANGELOG.md file
5. Add a workspace release in the workspace CHANGELOG.md file
6. Create a tag for each crate version
7. Create a PR from the release branch to the main branch
8. Merge PR to main
9. Publish crates to crates.io
10. Push the tags upstream
11. On the release branch increase the versions of all released crates to the next patch and develop version
12. Create a tag for the workspace release
13. Create and merge a PR to develop
14. Push the tags upstream

## Related projects and rationale

There was an attempt to use a modified version of [cargo-release](https://github.com/sunng87/cargo-release) but the opionions on the desired workflow currently suggest to build it out from scratch.
It would be nice to eventually consolidate both into a common project with enough flexibility to cover the union of the supported use-cases.

## Development

With the `nix-shell` you can run the test suite using:

```shell
nix-shell --run hc-release-automation-test
```
*/

#![allow(unused_imports)]
#![allow(dead_code)]

use anyhow::bail;
use anyhow::Context;
use comrak::{format_commonmark, parse_document, Arena, ComrakOptions};
use enumflags2::{bitflags, BitFlags};
use log::{debug, error, info, trace, warn};
use std::collections::{BTreeSet, HashSet};
use structopt::StructOpt;

pub(crate) mod changelog;
pub(crate) mod check;
pub(crate) mod common;
pub(crate) mod crate_;
pub(crate) mod crate_selection;
pub(crate) mod release;

#[cfg(test)]
pub(crate) mod tests;

use crate_selection::{aliases::CargoDepKind, CrateState, CrateStateFlags};
use release::ReleaseSteps;

type Fallible<T> = anyhow::Result<T>;
type CommandResult = Fallible<()>;

pub(crate) mod cli {
    use crate::crate_::CrateArgs;

    use super::*;
    use crate_selection::SelectionCriteria;
    use semver::Version;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[derive(Debug, StructOpt)]
    #[structopt(name = "release-automation")]
    pub(crate) struct Args {
        #[structopt(long)]
        pub(crate) workspace_path: PathBuf,

        #[structopt(subcommand)]
        pub(crate) cmd: Commands,

        #[structopt(long, default_value = "warn")]
        pub(crate) log_level: log::Level,
    }

    #[derive(Debug, StructOpt)]
    #[structopt(name = "ra")]
    pub(crate) enum Commands {
        Changelog(ChangelogArgs),
        Release(ReleaseArgs),
        Check(CheckArgs),
        Crate(CrateArgs),
    }

    #[derive(Debug, StructOpt)]
    pub(crate) struct ChangelogAggregateArgs {
        /// Allows a specified subset of crates that will haveh their changelog aggregated in the workspace changelog.
        /// This string will be used as a regex to filter the package names.
        /// By default, all crates will be considered.
        #[structopt(long, default_value = ".*")]
        pub(crate) match_filter: fancy_regex::Regex,

        /// Output path, relative to the workspace root.
        #[structopt(long, default_value = "CHANGELOG.md")]
        pub(crate) output_path: PathBuf,
    }

    #[derive(Debug, StructOpt)]
    pub(crate) enum ChangelogCommands {
        Aggregate(ChangelogAggregateArgs),
    }

    #[derive(StructOpt, Debug)]
    pub(crate) struct ChangelogArgs {
        #[structopt(subcommand)]
        pub(crate) command: ChangelogCommands,
    }

    /// Determine whether there are any release blockers by analyzing the state of the workspace.
    #[derive(StructOpt, Debug)]
    pub(crate) struct CheckArgs {
        /// All existing versions must match these requirements.
        /// Can be passed more than once to specify multiple.
        /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements
        #[structopt(long)]
        pub(crate) enforced_version_reqs: Vec<semver::VersionReq>,

        /// None of the existing versions are allowed to match these requirements.
        /// Can be passed more than once to specify multiple.
        /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements
        #[structopt(long)]
        pub(crate) disallowed_version_reqs: Vec<semver::VersionReq>,

        /// Allows a specified subset of crates to be released by regex matches on the crates' package name.
        /// This string will be used as a regex to filter the package names.
        /// By default, all crates will be considered release candidates.
        #[structopt(long, default_value = ".*")]
        pub(crate) match_filter: fancy_regex::Regex,

        /// Allow these blocking states for dev dependency crates.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub(crate) allowed_dev_dependency_blockers: BitFlags<CrateStateFlags>,

        /// Allow these blocking states for crates via the packages filter.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub(crate) allowed_matched_blockers: BitFlags<CrateStateFlags>,

        /// Exclude optional dependencies.
        #[structopt(long)]
        pub(crate) exclude_optional_deps: bool,
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

    impl CheckArgs {
        /// Boilerplate to instantiate `SelectionCriteria` from `CheckArgs`
        pub(crate) fn to_selection_criteria(&self) -> SelectionCriteria {
            SelectionCriteria {
                match_filter: self.match_filter.clone(),
                disallowed_version_reqs: self.disallowed_version_reqs.clone(),
                enforced_version_reqs: self.enforced_version_reqs.clone(),
                allowed_dev_dependency_blockers: self.allowed_dev_dependency_blockers,
                allowed_selection_blockers: self.allowed_matched_blockers,
                exclude_optional_deps: self.exclude_optional_deps,
            }
        }
    }

    /// Initiate a release process with the given arguments.
    ///
    /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements for details on the requirements arguments.
    #[derive(StructOpt, Debug)]
    pub(crate) struct ReleaseArgs {
        #[structopt(flatten)]
        pub(crate) check_args: CheckArgs,

        #[structopt(long)]
        pub(crate) dry_run: bool,

        /// Will be inferred from the current name if not given.
        #[structopt(long)]
        pub(crate) release_branch_name: Option<String>,

        /// The release steps to perform.
        /// These will be reordered to their defined ordering.
        ///
        /// See `ReleaseSteps` for the list of steps.
        #[structopt(long, default_value="", parse(try_from_str = parse_releasesteps))]
        pub(crate) steps: BTreeSet<ReleaseSteps>,

        /// Force creation of the branch regardless of source branch.
        #[structopt(long)]
        pub(crate) force_branch_creation: bool,

        /// Force creation of the git tags.
        #[structopt(long)]
        pub(crate) force_tag_creation: bool,

        /// The dependencies that are allowed to be missing at the search location despite not being released.
        #[structopt(long, default_value="", parse(from_str = parse_string_set))]
        pub(crate) allowed_missing_dependencies: HashSet<String>,
    }

    /// Parses a commad separated input string to a set of strings.
    pub(crate) fn parse_string_set(input: &str) -> HashSet<String> {
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
    pub(crate) fn parse_releasesteps(input: &str) -> Fallible<BTreeSet<ReleaseSteps>> {
        use std::str::FromStr;

        input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|csf| {
                ReleaseSteps::from_str(csf)
                    .map_err(|_| anyhow::anyhow!("could not parse '{}' as ReleaseSteps", input))
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

fn main() -> CommandResult {
    let args = cli::Args::from_args();

    env_logger::builder()
        .filter_level(args.log_level.to_level_filter())
        .filter(Some("cargo::core::workspace"), log::LevelFilter::Error)
        .format_timestamp(None)
        .init();

    debug!("args: {:#?}", args);

    match &args.cmd {
        cli::Commands::Changelog(cmd_args) => crate::changelog::cmd(&args, cmd_args),
        cli::Commands::Check(cmd_args) => crate::check::cmd(&args, cmd_args),
        cli::Commands::Release(cmd_args) => crate::release::cmd(&args, cmd_args),
        cli::Commands::Crate(cmd_args) => crate::crate_::cmd(&args, cmd_args),
    }
}
