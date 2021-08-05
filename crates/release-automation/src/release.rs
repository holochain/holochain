//! Release command functionality.

use super::*;

use anyhow::bail;
use anyhow::Context;
use bstr::ByteSlice;
use chrono::TimeZone;
use chrono::Utc;
use cli::ReleaseArgs;
use comrak::{format_commonmark, parse_document, Arena, ComrakOptions};
use enumflags2::{bitflags, BitFlags};
use log::{debug, error, info, trace, warn};
use once_cell::sync::OnceCell;
use std::convert::TryInto;
use std::iter::FromIterator;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::{
    collections::HashMap,
    io::{Read, Write},
};
use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};
use structopt::StructOpt;

use crate::changelog::{Changelog, WorkspaceCrateReleaseHeading};
use crate::crate_selection::Crate;
pub(crate) use crate_selection::{ReleaseWorkspace, SelectionCriteria};

/// These steps make up the release workflow
#[bitflags]
#[repr(u64)]
#[derive(enum_utils::FromStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReleaseSteps {
    /// create a new release branch based on develop
    CreateReleaseBranch,
    /// substeps: get crate selection, bump cargo toml versions, rotate
    /// changelog, commit changes
    BumpReleaseVersions,
    PushForPrToMain,
    CreatePrToMain,
    /// verify that the release tag exists on the main branch and is the
    /// second commit on it, directly after the merge commit
    VerifyMainBranch,
    PublishToCratesIo,
    AddOwnersToCratesIo,
    // CreateCrateTags,
    PushReleaseTag,
    PushForDevelopPr,
    CreatePrToDevelop,
}

// todo(backlog): what if at any point during the release process we have to merge a hotfix to main?
// todo: don't forget to adhere to dry-run into all of the following
/// This function handles the release process from start to finish.
/// Eventually this will be idempotent by understanding the state of the repository and
/// derive from it the steps that required to proceed with the release.
///
/// For now it is manual and the release phases need to be given as an instruction.
pub(crate) fn cmd(args: &crate::cli::Args, cmd_args: &crate::cli::ReleaseArgs) -> CommandResult {
    for step in &cmd_args.steps {
        trace!("Processing step '{:?}'", step);

        // read the workspace after every step in case it was mutated
        let ws = ReleaseWorkspace::try_new_with_criteria(
            args.workspace_path.clone(),
            cmd_args.check_args.to_selection_criteria(),
        )?;

        macro_rules! _skip_on_empty_selection {
            ($step:expr, $body:expr) => {
                if ws.release_selection()?.len() == 0 {
                    warn!("empty release selection. skipping {:?}", $step);
                } else {
                    $body
                }
            };
        }

        match step {
            ReleaseSteps::CreateReleaseBranch => create_release_branch(&ws, cmd_args)?,
            ReleaseSteps::BumpReleaseVersions => bump_release_versions(&ws, cmd_args)?,
            ReleaseSteps::PushForPrToMain => {
                // todo(backlog): push the release branch
                // todo(backlog): create a PR against the main branch
            }
            ReleaseSteps::CreatePrToMain => {
                // todo: create a pull request from the release branch to the main branch
                // todo: notify someone to review the PR
            }
            ReleaseSteps::VerifyMainBranch => {
                // todo: verify we're on the main branch
                // todo: verify the Pr has been merged
            }
            ReleaseSteps::PublishToCratesIo => publish_to_crates_io(&ws, cmd_args)?,
            ReleaseSteps::AddOwnersToCratesIo => {
                add_owners_to_crates_io(&ws, cmd_args, latest_release_crates(&ws)?)?
            }

            // ReleaseSteps::CreateCrateTags => create_crate_tags(&ws, cmd_args)?,
            ReleaseSteps::PushReleaseTag => {
                // todo: push all the tags that originated in this workspace release to the upstream:
                // - every crate release tag
            }
            ReleaseSteps::PushForDevelopPr => {
                // todo(backlog): push the release branch
            }
            ReleaseSteps::CreatePrToDevelop => {
                // todo(backlog): create a PR against the develop branch
                // todo: verify the Pr has been merged
            }
        }
    }

    Ok(())
}

pub(crate) const RELEASE_BRANCH_PREFIX: &str = "release-";

/// Generate a time-derived name for a new release branch.
pub(crate) fn generate_release_branch_name() -> String {
    format!(
        "{}{}",
        RELEASE_BRANCH_PREFIX,
        chrono::Utc::now().format("%Y%m%d.%H%M%S")
    )
}

/// Create a new git release branch.
pub(crate) fn create_release_branch<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    cmd_args: &ReleaseArgs,
) -> Fallible<()> {
    match ws.git_head_branch_name()?.as_str() {
        "develop" => {
            // we're good to continue!
        }
        _ if cmd_args.force_branch_creation => {}
        other => bail!(
            "only support releasing from the 'develop' branch, but found '{}'",
            other
        ),
    };

    let statuses = ws
        .git_repo()
        .statuses(Some(git2::StatusOptions::new().include_untracked(true)))
        .context("querying repository status")?;
    if !statuses.is_empty() && !cmd_args.force_branch_creation {
        bail!(
            "repository is not clean. {} change(s): \n{}",
            statuses.len(),
            statuses
                .iter()
                .map(|statusentry| format!(
                    "{:?}: {}\n",
                    statusentry.status(),
                    statusentry.path().unwrap_or_default()
                ))
                .collect::<String>()
        )
    };

    let release_branch_name = cmd_args
        .release_branch_name
        .to_owned()
        .unwrap_or_else(generate_release_branch_name);

    if cmd_args.dry_run {
        info!("[dry-run] would create branch '{}'", release_branch_name);
    } else {
        ws.git_checkout_new_branch(&release_branch_name)?;

        ensure_release_branch(ws)?;
    }

    Ok(())
}

fn bump_release_versions<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    cmd_args: &'a ReleaseArgs,
) -> Fallible<()> {
    let branch_name = match ensure_release_branch(ws) {
        Ok(branch_name) => branch_name,
        Err(_) if cmd_args.dry_run => generate_release_branch_name(),
        Err(e) => bail!(e),
    };

    // check the workspace and determine the release selection
    // todo: double-check that we select matching cratese that had their dependencies change
    let selection = crate::common::selection_check(&cmd_args.check_args, ws)?;

    if selection.is_empty() {
        debug!("no crates to release, exiting.");
        return Ok(());
    }

    let mut changed_crate_changelogs = vec![];

    for crt in &selection {
        let current_version = crt.version();
        let maybe_previous_release_version = crt
            .changelog()
            .ok_or_else(|| {
                anyhow::anyhow!("[{}] cannot determine most recent release: missing changelog")
            })?
            .topmost_release()?
            .map(|change| semver::Version::parse(change.title()))
            .transpose()?;

        let release_version = if let Some(mut previous_release_version) =
            maybe_previous_release_version.clone()
        {
            if previous_release_version > current_version {
                bail!("previously documented release version '{}' is greater than this release version '{}'", previous_release_version, current_version);
            }

            // todo(backlog): support configurable major/minor/patch/rc? version bumps
            previous_release_version.increment_patch();

            previous_release_version
        } else {
            // release the current version, or bump if the current version is a pre-release
            let mut new_version = current_version.clone();

            if new_version.is_prerelease() {
                // todo(backlog): support configurable major/minor/patch/rc? version bumps
                new_version.increment_patch();
            }

            new_version
        };

        trace!(
            "[{}] previous release version: '{:?}', current version: '{}', release version: '{}' ",
            crt.name(),
            maybe_previous_release_version,
            current_version,
            release_version,
        );

        let greater_release = release_version > current_version;
        if greater_release {
            common::set_version(cmd_args.dry_run, crt, &release_version.clone())?;
        }

        let crate_release_heading_name = format!("{}", release_version);

        if maybe_previous_release_version.is_none() || greater_release {
            // create a new release entry in the crate's changelog and move all items from the unreleased heading if there are any

            let changelog = crt
                .changelog()
                .ok_or_else(|| anyhow::anyhow!("{} doesn't have changelog", crt.name()))?;

            debug!(
                "[{}] creating crate release heading '{}' in '{:?}'",
                crt.name(),
                crate_release_heading_name,
                changelog.path(),
            );

            if !cmd_args.dry_run {
                changelog
                    .add_release(crate_release_heading_name.clone())
                    .context(format!("adding release to changelog for '{}'", crt.name()))?;
            }

            changed_crate_changelogs.push(WorkspaceCrateReleaseHeading {
                prefix: crt.name(),
                suffix: crate_release_heading_name,
                changelog,
            });
        }
    }

    // ## for the workspace release:
    let workspace_release_name = branch_name
        .strip_prefix(RELEASE_BRANCH_PREFIX)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "expected branch name to start with prefix '{}'. got instead: {}",
                RELEASE_BRANCH_PREFIX,
                branch_name,
            )
        })?
        .to_string();

    let ws_changelog = ws
        .changelog()
        .ok_or_else(|| anyhow::anyhow!("workspace has no changelog"))?;

    info!(
        "adding release {} to changelog at {:?} with the following crate releases: {}",
        workspace_release_name,
        ws_changelog.path(),
        changed_crate_changelogs
            .iter()
            .map(|cr| format!("\n- {}", cr.title()))
            .collect::<String>()
    );

    if !cmd_args.dry_run {
        ws_changelog.add_release(workspace_release_name, &changed_crate_changelogs)?;
    }

    info!("running `cargo publish --dry-run --allow-dirty ..` for all selected crates...");
    publish_paths_to_crates_io(
        &selection,
        true,
        true,
        &cmd_args.allowed_missing_dependencies,
    )
    .context("running 'cargo publish' in dry-run mode for all selected crates")?;

    // create a release commit with an overview of which crates are included
    let commit_msg = indoc::formatdoc!(
        r#"
        create a release from branch {}

        the following crates are part of this release:
        {}
        "#,
        branch_name,
        changed_crate_changelogs
            .iter()
            .map(|wcrh| format!("\n- {}", wcrh.title()))
            .collect::<String>()
    );

    info!("creating the following commit: {}", commit_msg);
    if !cmd_args.dry_run {
        // this checks consistency and also updates the Cargo.lock file(s)
        ws.cargo_check()?;

        ws.git_add_all_and_commit(&commit_msg, None)?;
    };

    // create tags for all released crates
    let tags_to_create = changed_crate_changelogs
        .iter()
        .map(|wcrh| wcrh.title())
        .collect::<Vec<String>>();
    create_crate_tags(&ws, tags_to_create, cmd_args)?;

    Ok(())
}

fn publish_to_crates_io<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    cmd_args: &'a ReleaseArgs,
) -> Fallible<()> {
    let crates = latest_release_crates(ws)?;

    publish_paths_to_crates_io(&crates, cmd_args.dry_run, false, &Default::default())?;

    Ok(())
}

fn add_owners_to_crates_io<'a>(
    _ws: &'a ReleaseWorkspace<'a>,
    cmd_args: &'a ReleaseArgs,
    crates: Vec<&Crate>,
) -> Fallible<()> {
    // TODO(backlog): make this configurable
    static DEFAULT_CRATE_OWNERS: &[&str] = &["github:holochain:core-dev", "zippy"];

    let desired_owners = DEFAULT_CRATE_OWNERS
        .iter()
        .map(|s| s.to_string())
        .collect::<HashSet<_>>();

    for crt in crates {
        if crates_index_helper::is_version_published(crt, false)? {
            let mut cmd = std::process::Command::new("cargo");
            cmd.args(&["owner", "--list", &crt.name()]);

            debug!("[{}] running command: {:?}", crt.name(), cmd);
            let output = cmd.output().context("process exitted unsuccessfully")?;
            if !output.status.success() {
                warn!(
                    "[{}] failed list owners: {}",
                    crt.name(),
                    String::from_utf8_lossy(&output.stderr)
                );

                continue;
            }

            let current_owners = output
                .stdout
                .lines()
                .map(|line| {
                    line.words_with_breaks()
                        .take_while(|item| *item != " ")
                        .collect::<String>()
                })
                .collect::<HashSet<_>>();
            let diff = desired_owners.difference(&current_owners);
            info!(
                "[{}] current owners {:?}, missing owners: {:?}",
                crt.name(),
                current_owners,
                diff
            );

            for owner in diff {
                let mut cmd = std::process::Command::new("cargo");
                cmd.args(&["owner", "--add", owner, &crt.name()]);

                debug!("[{}] running command: {:?}", crt.name(), cmd);
                if !cmd_args.dry_run {
                    let output = cmd.output().context("process exitted unsuccessfully")?;
                    if !output.status.success() {
                        warn!(
                            "[{}] failed to add owner '{}': {}",
                            crt.name(),
                            owner,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn latest_release_crates<'a>(ws: &'a ReleaseWorkspace<'a>) -> Fallible<Vec<&Crate>> {
    let (release_title, crate_release_titles) = match ws
        .changelog()
        .map(|cl| cl.topmost_release())
        .transpose()?
        .flatten()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no topmost release found in changelog '{:?}'. nothing to publish",
                ws.changelog()
            )
        })? {
        changelog::ReleaseChange::WorkspaceReleaseChange(title, releases) => {
            (title, releases.into_iter().collect::<BTreeSet<_>>())
        }
        unexpected => bail!("unexpected topmost release: {:?}", unexpected),
    };
    debug!("{}: {:#?}", release_title, crate_release_titles);

    let crates = ws
        .members()?
        .iter()
        .filter_map(|member| {
            if crate_release_titles.contains(&member.name_version()) {
                Some(*member)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(crates)
}

/// This models the information in the failure output of `cargo publish --dry-run`.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub(crate) enum PublishError {
    #[error(
        "{package}@{path}: '{dependency}' dependency by {package_found} not found at {location}"
    )]
    PackageNotFound {
        package: String,
        version: String,
        path: String,
        dependency: String,
        package_found: String,
        location: String,
    },
    #[error(
        "{package}@{path}: '{dependency} = \"{version_req}\"' dependency by {package_found}-{version_found} not found at {location}"
    )]
    PackageVersionNotFound {
        package: String,
        version: String,
        path: String,
        dependency: String,
        version_req: String,
        location: String,
        package_found: String,
        version_found: String,
    },
    #[error("{package}@{path}: {version} already uploaded at {location}")]
    AlreadyUploaded {
        package: String,
        version: String,
        path: String,
        location: String,
        version_found: String,
    },

    #[error("{package}: publish rate limit exceeded. retry after '{retry_after}'")]
    PublishLimitExceeded {
        package: String,
        version: String,
        location: String,
        retry_after: chrono::DateTime<Utc>,
    },

    #[error("{}: {}", _0, _1)]
    Other(String, String),
}

impl PublishError {
    pub(crate) fn with_str(package: String, version: String, input: String) -> Self {
        static PACKAGE_NOT_FOUND_RE: OnceCell<regex::Regex> = OnceCell::new();
        static PACKAGE_VERSION_NOT_FOUND_RE: OnceCell<regex::Regex> = OnceCell::new();
        static ALREADY_UPLOADED_RE: OnceCell<regex::Regex> = OnceCell::new();
        static PUBLISH_LIMIT_EXCEEDED_RE: OnceCell<regex::Regex> = OnceCell::new();

        if let Some(captures) = PACKAGE_NOT_FOUND_RE
            .get_or_init(|| {
                regex::Regex::new(indoc::indoc!(
                    r#"
                    (.*"(?P<path>.*)":)?
                    (.|\n)*
                    .*no matching package named `(?P<dependency>.*)` found
                    .*location searched: (?P<location>.*)
                    .*required by package `(?P<package>\S+) v(?P<version>\S+).*`
                    "#
                ))
                .expect("regex should compile")
            })
            .captures(&input)
        {
            if let (path, Some(dependency), Some(location), Some(package_found), Some(version_found)) = (
                captures.name("path"),
                captures.name("dependency"),
                captures.name("location"),
                captures.name("package"),
                captures.name("version"),
            ) {
                    let package_found= package_found.as_str().to_string();
                    if package_found != package {
                        warn!("package mismatch. got '{}' expected '{}'", package_found, package);
                    }
                    let version_found= version_found.as_str().to_string();
                    if version_found != version {
                        warn!("version mismatch. got '{}' expected '{}'", version_found, version);
                    }
                    return PublishError::PackageNotFound {
                        package,
                        version,
                        path: path.map(|path| path.as_str().to_string()).unwrap_or_default(),
                        dependency: dependency.as_str().to_string(),
                        location: location.as_str().to_string(),
                        package_found,
                    }
                }
        } else if let Some(captures) = PACKAGE_VERSION_NOT_FOUND_RE
            .get_or_init(|| {
                regex::Regex::new(indoc::indoc!(
                    r#"
                        (.*"(?P<path>.*)":)?
                        (.|\n)*
                        .*failed to select a version for the requirement `(?P<dependency>.*) = "(?P<version_req>.*)"`
                        .*
                        .*location searched: (?P<location>.*)
                        .*required by package `(?P<package>\S+) v(?P<version>\S+).*`
                    "#
                ))
                .expect("regex should compile")
            })
            .captures(&input)
        {
            if let                 (path, Some(dependency), Some(version_req), Some(location), Some(package_found), Some(version_found))
            = (
                captures.name("path"),
                captures.name("dependency"),
                captures.name("version_req"),
                captures.name("location"),
                captures.name("package"),
                captures.name("version"),
            ) {
                let package_found= package_found.as_str().to_string();
                if package_found != package {
                    warn!("package mismatch. got '{}' expected '{}'", package_found, package);
                }

                let version_found= version_found.as_str().to_string();
                if version_found != version {
                    warn!("version mismatch. got '{}' expected '{}'", version_found, version);
                }
                return PublishError::PackageVersionNotFound {
                    package,
                    version,
                    path: path.map(|path| path.as_str().to_string()).unwrap_or_default(),
                    dependency: dependency.as_str().to_string(),
                    version_req: version_req.as_str().to_string(),
                    location: location.as_str().to_string(),
                    package_found,
                    version_found,
                }
            }

        } else if let Some(captures) = ALREADY_UPLOADED_RE
            .get_or_init(|| {
                regex::Regex::new(indoc::indoc!(
                    r#"
                    error: failed to publish to (?P<location>.*)
                    (.|\n)*
                    .*crate version `(?P<version>.*)` is already uploaded
                    "#
                ))
                .expect("regex should compile")
            })
            .captures(&input)
        {
            if let (path, Some(location), Some(version_found)) = (
                captures.name("path"),
                captures.name("location"),
                captures.name("version"),
            ) {
                let version_found= version_found.as_str().to_string();
                if version_found != version {
                    warn!("version mismatch. got '{}' expected '{}'", version_found, version);
                }
                return PublishError::AlreadyUploaded {
                    package,
                    version,
                    path: path.map(|path| path.as_str().to_string()).unwrap_or_default(),
                    location: location.as_str().to_string(),
                    version_found,
                }
            }
        } else if let Some(captures) = PUBLISH_LIMIT_EXCEEDED_RE
            .get_or_init(|| {
                regex::Regex::new(indoc::indoc!(
                    r#"
                    error: failed to publish to (?P<location>.*)
                    (.|\n)*
                    .*try again after (?P<retry_after>.*) or.*
                    "#
                ))
                .expect("regex should compile")
            })
            .captures(&input)
        {
            if let (Some(location), Some(retry_after_string)) = (
                captures.name("location"),
                captures.name("retry_after"),
            ) {
                let retry_after =
                chrono::Utc.timestamp(
                    chrono::DateTime::parse_from_rfc2822(retry_after_string.as_str())
                    .expect("time to parse").timestamp(), 0);
                return PublishError::PublishLimitExceeded{
                    package,
                    version,
                    location: location.as_str().to_string(),
                    retry_after,
                }
            }
        }

        PublishError::Other(package, input)
    }
}

mod crates_index_helper {
    use super::*;

    static CRATES_IO_INDEX: OnceCell<crates_index::Index> = OnceCell::new();

    pub(crate) fn index(update: bool) -> Fallible<&'static crates_index::Index> {
        let first_run = CRATES_IO_INDEX.get().is_none();

        let crates_io_index = CRATES_IO_INDEX.get_or_try_init(|| -> Fallible<_> {
            let index = crates_index::Index::new_cargo_default();
            trace!("Using crates index at {:?}", index.path());

            index.retrieve_or_update()?;

            Ok(index)
        })?;

        if !first_run && update {
            crates_io_index.update()?;
        }

        Ok(crates_io_index)
    }

    pub(crate) fn is_version_published(crt: &Crate, update: bool) -> Fallible<bool> {
        Ok(index(update)?
            .crate_(&crt.name())
            .map(|indexed_crate| -> bool {
                indexed_crate
                    .versions()
                    .iter()
                    .any(|version| crt.version().to_string() == version.version())
            })
            .unwrap_or_default())
    }
}

/// Try to publish the given manifests to crates.io.
///
/// If dry-run is given, the following error conditoins are tolerated:
/// - a dependency is not found but is part of the release
/// - a version of a dependency is not found bu the dependency is part of the release
fn publish_paths_to_crates_io(
    crates: &[&Crate],
    dry_run: bool,
    allow_dirty: bool,
    allowed_missing_dependencies: &HashSet<String>,
) -> Fallible<()> {
    static USER_AGENT: &str = "Holochain_Core_Dev_Team (devcore@holochain.org)";
    static CRATES_IO_CLIENT: OnceCell<crates_io_api::AsyncClient> = OnceCell::new();

    let crate_names: HashSet<String> = crates.iter().map(|crt| crt.name()).collect();

    let mut queue = crates.iter().collect::<std::collections::LinkedList<_>>();
    let mut errors: Vec<PublishError> = vec![];
    while let Some(crt) = queue.pop_front() {
        if !dry_run && crates_index_helper::is_version_published(crt, false)? {
            debug!("{} is already published, skipping..", crt.name_version());
            continue;
        }

        let mut cmd = std::process::Command::new("cargo");

        let path = crt.manifest_path();

        cmd.args(
            [
                vec!["publish"],
                if dry_run {
                    vec!["--dry-run", "--no-verify"]
                } else {
                    vec![]
                },
                if allow_dirty {
                    vec!["--allow-dirty"]
                } else {
                    vec![]
                },
                vec![
                    // "--no-default-features",
                    "--verbose",
                    &format!("--manifest-path={}", path.to_string_lossy()),
                ],
            ]
            .concat(),
        );

        debug!("Running command: {:?}", cmd);

        let output = cmd.output().context("process exitted unsuccessfully")?;
        if !output.status.success() {
            let mut details = String::new();
            for line in output.stderr.lines_with_terminator() {
                let line = line.to_str_lossy();
                details += &line;
            }

            let error = PublishError::with_str(crt.name(), crt.version().to_string(), details);

            if match &error {
                PublishError::Other(..) => true,
                PublishError::PackageNotFound { dependency, .. }
                | PublishError::PackageVersionNotFound { dependency, .. } => {
                    !dry_run
                        || !(crate_names.contains(dependency)
                            || allowed_missing_dependencies.contains(dependency))
                }
                PublishError::AlreadyUploaded { version, .. } => {
                    crt.version().to_string() != *version
                }
                PublishError::PublishLimitExceeded { retry_after, .. } => {
                    let wait = *retry_after - chrono::offset::Utc::now();
                    warn!("waiting for {:?} to adhere to the rate limit...", wait);
                    std::thread::sleep(wait.to_std()?);
                    queue.push_front(crt);
                    continue;
                }
            } {
                errors.push(error);
            } else {
                trace!("tolerating error: '{:#?}'", &error);
            }
        } else if !dry_run {
            // wait until the published version is live

            let mut found = false;

            for delay_secs in &[56, 28, 14, 7, 14, 28, 56] {
                let duration = std::time::Duration::from_secs(*delay_secs);
                std::thread::sleep(duration);

                if crates_index_helper::is_version_published(crt, true)? {
                    debug!(
                        "Found recently published {} on crates.io!",
                        crt.name_version()
                    );
                    found = true;
                    break;
                }

                debug!(
                    "Did not find {} on crates.io, retrying in {:?}...",
                    crt.name_version(),
                    duration
                );
            }

            if !found {
                bail!(
                    "recently published version of {} not found in time on the crates_io index",
                    crt.name_version()
                );
            }
        }
    }

    if !errors.is_empty() {
        let mut root = anyhow::anyhow!("cargo publish failed for at least one manifest");
        for error in errors.into_iter().rev() {
            root = root.context(error);
        }
        return Err(root);
    }

    Ok(())
}

/// create a tag for each crate which will be used to identify its latest release
fn create_crate_tags<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    tags_to_create: Vec<String>,
    cmd_args: &'a ReleaseArgs,
) -> Fallible<()> {
    let existing_tags = tags_to_create
        .iter()
        .filter_map(|git_tag| crate::crate_selection::git_lookup_tag(ws.git_repo(), &git_tag))
        .collect::<Vec<_>>();

    if !cmd_args.force_tag_creation && !existing_tags.is_empty() {
        error!(
            "the following tags already exist: {}",
            existing_tags
                .iter()
                .map(|tag| format!("\n- {}", tag))
                .collect::<String>()
        )
    }

    for git_tag in tags_to_create {
        debug!("creating tag '{}'", git_tag);
        if !cmd_args.dry_run {
            ws.git_tag(&git_tag, cmd_args.force_tag_creation)?;
        }
    }

    Ok(())
}

/// Ensure we're on a branch that starts with `Self::RELEASE_BRANCH_PREFIX`
pub(crate) fn ensure_release_branch<'a>(ws: &'a ReleaseWorkspace<'a>) -> Fallible<String> {
    let branch_name = ws.git_head_branch_name()?;
    if !branch_name.starts_with(RELEASE_BRANCH_PREFIX) {
        bail!(
            "expected branch name with prefix '{}', got '{}'",
            RELEASE_BRANCH_PREFIX,
            branch_name
        );
    }

    Ok(branch_name)
}
