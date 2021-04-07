#![allow(unused_imports)]
#![allow(dead_code)]


use comrak::{format_commonmark, parse_document, Arena, ComrakOptions};
use enumflags2::BitFlags;
use log::{debug, error, info, trace, warn};
use std::collections::HashSet;
use structopt::StructOpt;

pub(crate) mod changelog;
pub(crate) mod crate_selection;

#[cfg(test)]
pub(crate) mod tests;

use crate_selection::{aliases::CargoDepKind, CrateState, CrateStateFlags};

type Fallible<T> = anyhow::Result<T>;
type CommandResult = Fallible<()>;


pub(crate) mod cli {
    use super::*;
    use anyhow::bail;
    use crate_selection::SelectionCriteria;
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use structopt::StructOpt;

    #[derive(Debug, StructOpt)]
    #[structopt(name = "release-automation")]
    pub(crate) struct Args {
        #[structopt(long)]
        workspace_path: PathBuf,

        #[structopt(subcommand)]
        pub(crate) cmd: Commands,

        #[structopt(long, default_value = "warn")]
        pub(crate) log_level: log::Level,
    }

    #[derive(Debug, StructOpt)]
    #[structopt(name = "ra")]
    pub(crate) enum Commands {
        Changelog(Changelog),
        Members(Members),
        Release(Release),
        Check(Check),
    }

    #[derive(Debug, StructOpt)]
    pub(crate) struct AggregateChangelogs {}

    #[derive(Debug, StructOpt)]
    enum ChangelogCommands {
        Aggregate(AggregateChangelogs),
    }

    #[derive(StructOpt, Debug)]
    pub(crate) struct Changelog {
        #[structopt(subcommand)]
        command: ChangelogCommands,
    }

    #[derive(StructOpt, Debug)]
    pub(crate) struct Members {}

    /// Determine whether there are any release blockers by analyzing the state of the workspace.
    #[derive(StructOpt, Debug)]
    pub(crate) struct Check {
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
        pub(crate) selection_filter: fancy_regex::Regex,

        /// Allow these blocking states for dependency crates.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub(crate) allowed_dependency_blockers: BitFlags<CrateStateFlags>,

        /// Allow these blocking states for crates via the packages filter.
        /// Comma separated.
        /// Valid values are: MissingReadme, UnreleasableViaChangelogFrontmatter, DisallowedVersionReqViolated, EnforcedVersionReqViolated
        #[structopt(long, default_value = "", parse(try_from_str = parse_cratestateflags))]
        pub(crate) allowed_selection_blockers: BitFlags<CrateStateFlags>,

        /// These dependency types will be ignored.
        /// Comma separated.
        /// Valid values are: normal, build, development
        #[structopt(long, default_value="", parse(try_from_str = parse_depkind))]
        pub(crate) exclude_dep_kinds: HashSet<CargoDepKind>,

        /// Exclude optional dependencies.
        #[structopt(long)]
        pub(crate) exclude_optional_deps: bool,
    }

    fn parse_depkind(input: &str) -> Fallible<HashSet<CargoDepKind>> {
        let mut set = HashSet::new();

        for word in input.split(",") {
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
            .split(",")
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

    impl Check {
        fn criteria(&self) -> SelectionCriteria {
            SelectionCriteria {
                selection_filter: self.selection_filter.clone(),
                disallowed_version_reqs: self.disallowed_version_reqs.clone(),
                enforced_version_reqs: self.enforced_version_reqs.clone(),
                allowed_dependency_blockers: self.allowed_dependency_blockers.clone(),
                allowed_selection_blockers: self.allowed_selection_blockers.clone(),
                exclude_dep_kinds: self.exclude_dep_kinds.clone(),
                exclude_optional_deps: self.exclude_optional_deps,
            }
        }
    }

    /// Initiate a release process with the given arguments.
    ///
    /// See https://docs.rs/semver/0.11.0/semver/?search=#requirements for details on the requirements arguments.
    #[derive(StructOpt, Debug)]
    pub(crate) struct Release {
        #[structopt(flatten)]
        check_args: Check,

        #[structopt(long)]
        pub(crate) dry_run: bool,
    }

    impl Release {
        fn criteria(&self) -> SelectionCriteria {
            // todo

            // let mut allowed_blocking_states = BitFlags::<CrateStateFlags>::empty();
            // if self.allow_missing_changelog {
            //     allowed_blocking_states.insert(CrateStateFlags::MissingChangelog)
            // }

            SelectionCriteria {
                // selection_filter: self.selection_filter.clone(),
                // disallowed_version_reqs: self.disallowed_version_reqs.clone(),
                // enforced_version_reqs: self.enforced_version_reqs.clone(),
                // allowed_blocking_states,
                ..Default::default()
            }
        }
    }

    pub(crate) fn changelog(_args: &Args, cmd_args: &Changelog) -> CommandResult {
        debug!("cmd_args: {:#?}", cmd_args);

        bail!("todo")
    }

    pub(crate) fn members(args: &Args, cmd_args: &Members) -> CommandResult {
        debug!("cmd_args: {:#?}", cmd_args);
        let ws = crate_selection::ReleaseWorkspace::try_new(args.workspace_path.clone())?;

        debug!("{:#?}", ws.members()?);

        Ok(())
    }

    fn do_check<'a>(
        _args: &Args,
        cmd_args: &'a Check,
        ws: &'a crate_selection::ReleaseWorkspace<'a>,
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

    pub(crate) fn check<'a>(args: &Args, cmd_args: &Check) -> CommandResult {
        let ws = crate_selection::ReleaseWorkspace::try_new_with_criteria(
            args.workspace_path.clone(),
            cmd_args.criteria(),
        )?;

        let release_candidates = do_check(args, cmd_args, &ws)?;

        println!(
            "{}",
            crate_selection::CrateState::format_crates_states(
                &release_candidates
                    .iter()
                    .map(|member| (member.name(), member.state()))
                    .collect::<Vec<_>>(),
                "The following crates would have been selected for the release process.",
                false,
                true,
                false,
            )
        );

        Ok(())
    }

    // todo(backlog): what if at any point during the release process we have to merge a hotfix to main?
    pub(crate) fn release(args: &Args, cmd_args: &Release) -> CommandResult {
        let ws = crate_selection::ReleaseWorkspace::try_new_with_criteria(
            args.workspace_path.clone(),
            cmd_args.criteria(),
        )?;

        // # phase 0 - release initiation
        // todo: error if the branch doesn't begin with "release-"?

        // check the workspace and determine the release selection
        let _selection = do_check(args, &cmd_args.check_args, &ws)?;

        // ## per-crate initial steps. for every selected crate:
        // todo: bump the selected crate in the Cargo.toml to the next patch version
        // todo(backlog): support configurable major/minor/patch/rc? version bumps
        // todo: create a new release entry in the crate' changelog and move all items from the unreleased heading if there are any
        // todo: create a commit for the crate release
        // todo: create a tag for the crate release

        // ## for the workspace release:
        // todo: aggregate the changelogs into the toplevel one
        // todo: create a release commit with an overview of which crates are included
        // todo: crate a tag for the workspace release

        // todo(backlog): push the release branch
        // todo(backlog): create a PR against the main branch

        // # phase 1 - changes for the main branch and publish
        // todo: verify we're on the main branch
        // todo: wait for PR to main to be merged
        // todo: try to publish the crates to crates.io and create a new tag for every published crate
        // todo: push all the tags that originated in this workspace release to the upstream
        // todo: for each newly published crate add `github:holochain:core-dev` and `zippy` as an owner on crates.io

        // # phase 2 - changes for the develop branch
        // todo: bump versions for every released crate to the next develop version. create a commit for reach crate
        // todo: create a commit that concludes the workspace release?

        // todo(backlog): push the release branch
        // todo(backlog): create a PR against the develop branch

        bail!("todo")
    }
}

fn main() -> CommandResult {
    let args = cli::Args::from_args();

    env_logger::builder()
        .filter_level(args.log_level.to_level_filter())
        .format_timestamp(None)
        .init();

    debug!("args: {:#?}", args);

    match &args.cmd {
        cli::Commands::Changelog(cmd_args) => cli::changelog(&args, cmd_args),
        cli::Commands::Members(cmd_args) => cli::members(&args, cmd_args),
        cli::Commands::Check(cmd_args) => cli::check(&args, cmd_args),
        cli::Commands::Release(cmd_args) => cli::release(&args, cmd_args),
    }
}
