//! Release command functionality.

use super::*;

use anyhow::bail;
use anyhow::Context;
use bstr::ByteSlice;
use cargo::util::VersionExt;
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

use crate::{
    changelog::{Changelog, WorkspaceCrateReleaseHeading},
    common::{increment_semver, SemverIncrementMode},
    crate_::ensure_crate_io_owners,
    crate_selection::{ensure_release_order_consistency, Crate},
};
pub use crate_selection::{ReleaseWorkspace, SelectionCriteria};

const TARGET_DIR_SUFFIX: &str = "target/release_automation";

/// These steps make up the release workflow
#[bitflags]
#[repr(u64)]
#[derive(enum_utils::FromStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseSteps {
    /// create a new release branch based on develop
    CreateReleaseBranch,
    /// substeps: get crate selection, bump cargo toml versions, rotate
    /// changelog, commit changes
    BumpReleaseVersions,
    /// verify that the release tag exists on the main branch and is the
    /// second commit on it, directly after the merge commit
    PublishToCratesIo,
    AddOwnersToCratesIo,
}

// todo(backlog): what if at any point during the release process we have to merge a hotfix to main?
// todo: don't forget to adhere to dry-run into all of the following
/// This function handles the release process from start to finish.
/// Eventually this will be idempotent by understanding the state of the repository and
/// derive from it the steps that required to proceed with the release.
///
/// For now it is manual and the release phases need to be given as an instruction.
pub fn cmd(args: &crate::cli::Args, cmd_args: &crate::cli::ReleaseArgs) -> CommandResult {
    for step in &cmd_args.steps {
        trace!("Processing step '{:?}'", step);

        // read the workspace after every step in case it was mutated
        let ws = ReleaseWorkspace::try_new_with_criteria(
            args.workspace_path.clone(),
            cmd_args.check_args.to_selection_criteria(args),
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
            ReleaseSteps::PublishToCratesIo => publish_to_crates_io(&ws, cmd_args)?,
            ReleaseSteps::AddOwnersToCratesIo => ensure_crate_io_owners(
                &ws,
                cmd_args.dry_run,
                &latest_release_crates(&ws)?,
                &cmd_args.minimum_crate_owners,
            )?,
        }
    }

    Ok(())
}

pub const RELEASE_BRANCH_PREFIX: &str = "release-";

/// Generate a time-derived name for a new release branch.
pub fn generate_release_branch_name() -> String {
    format!(
        "{}{}",
        RELEASE_BRANCH_PREFIX,
        chrono::Utc::now().format("%Y%m%d.%H%M%S")
    )
}

/// Create a new git release branch.
pub fn create_release_branch<'a>(
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

    // run the checks to ensure the repo is in a consistent state to begin with
    if !cmd_args.no_verify && !cmd_args.no_verify_pre {
        info!("running consistency checks before changing the versions...");
        do_publish_to_crates_io(
            &selection,
            true,
            true,
            false,
            &cmd_args.allowed_missing_dependencies,
            &cmd_args.cargo_target_dir,
        )
        .context("consistency checks failed")?;
    }

    let mut changed_crate_changelogs = vec![];

    for crt in &selection {
        let current_version = crt.version();
        let changelog = crt
            .changelog()
            .ok_or_else(|| anyhow::anyhow!("[{}] missing changelog", crt.name()))?;

        let maybe_previous_release_version = changelog
            .topmost_release()?
            .map(|change| semver::Version::parse(change.title()))
            .transpose()
            .context(format!(
                "parsing {:#?} in {:#?} as a semantic version",
                changelog.topmost_release(),
                changelog.path(),
            ))?;

        let maybe_semver_increment_mode = changelog
            .front_matter()?
            .map(|fm| fm.semver_increment_mode());
        let semver_increment_mode = maybe_semver_increment_mode.unwrap_or_default();

        let incremented_version = {
            let mut v = current_version.clone();
            increment_semver(&mut v, semver_increment_mode)?;
            v
        };

        let release_version = match &maybe_previous_release_version {
            Some(previous_release_version) => {
                if &current_version > previous_release_version {
                    current_version.clone()
                } else if &incremented_version > previous_release_version {
                    crt.set_version(cmd_args.dry_run, &incremented_version)?;
                    incremented_version.clone()
                } else {
                    bail!("neither current version '{}' nor incremented version '{}' exceed previously released version '{}'", &current_version, &incremented_version, previous_release_version);
                }
            }

            None => {
                // default to incremented version if we don't have information on a previous release
                crt.set_version(cmd_args.dry_run, &incremented_version.clone())?;
                incremented_version.clone()
            }
        };

        debug!(
            "[{}] previous release version: '{:?}', current version: '{}', incremented version: '{}'",
            crt.name(),
            maybe_previous_release_version,
            current_version,
            incremented_version,
        );

        let crate_release_heading_name = format!("{}", release_version);

        // create a new release entry in the crate's changelog and move all items from the unreleased heading if there are any
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

            // FIXME: now we should reread the whole thing?

            // rewrite frontmatter to reset it to its defaults
            changelog.reset_front_matter_to_defaults()?;
        }

        changed_crate_changelogs.push(WorkspaceCrateReleaseHeading {
            prefix: crt.name(),
            suffix: crate_release_heading_name,
            changelog,
        });
    }

    ws.update_lockfile(
        cmd_args.dry_run,
        cmd_args.additional_manifests.iter().map(|mp| mp.as_str()),
    )?;

    /* TODO: the workspace probably needs to be re-read here because otherwise the publish dry-run will assume the previous crate versions
     * either this or something else is leading to this issue where the verify_post checks aren't effective
     *
     * > [INFO  release_automation::lib::common] crates selected for the release process: [
     * >         "holochain_cli-0.1.0-a-minor-release-test.2",
     * >     ]
     * > [DEBUG release_automation::lib::crate_selection] setting version to 0.1.0-a-minor-release-test.3 in manifest at "/home/steveej/src/holo/holochain/crates/hc/Cargo.toml"
     * > [DEBUG release_automation::lib::release] [holochain_cli] creating crate release heading '0.1.0-a-minor-release-test.3' in '"/home/steveej/src/holo/holochain/crates/hc/CHANGELOG.md"'
     * > [DEBUG release_automation::lib::crate_selection] running command: "cargo" "fetch" "--verbose" "--manifest-path" "Cargo.toml"
     * > [DEBUG release_automation::lib::crate_selection] running command: "cargo" "update" "--workspace" "--offline" "--verbose"
     * > [DEBUG release_automation::lib::crate_selection] running command: "cargo" "fetch" "--verbose" "--manifest-path" "crates/test_utils/wasm/wasm_workspace/Cargo.toml"
     * >     Blocking waiting for file lock on package cache
     * >     Blocking waiting for file lock on package cache
     * > [DEBUG release_automation::lib::crate_selection] running command: "cargo" "update" "--workspace" "--offline" "--verbose" "--manifest-path" "crates/test_utils/wasm/wasm_workspace/Cargo.toml"
     * >     Blocking waiting for file lock on package cache
     * > [INFO  release_automation::lib::release] running consistency checks after changing the versions...
     * > [DEBUG release_automation::lib::release] attempting to publish {"holochain_cli"}
     * > [DEBUG release_automation::lib::release] holochain_cli-0.1.0-a-minor-release-test.2 is unchanged and already published, skipping..
     * > [DEBUG release_automation::lib::release] Running command: "cargo" "check" "--locked" "--verbose" "--release" "--manifest-path=/home/steveej/src/holo/holochain/crates/hc/Cargo.toml"
     * > [DEBUG release_automation::lib::release] Running command: "cargo" "publish" "--locked" "--verbose" "--no-verify" "--manifest-path=/home/steveej/src/holo/holochain/crates/hc/Cargo.toml" "--dry-run" "--allow-dirty"
     * > [INFO  release_automation::lib::release] successfully published holochain_cli-0.1.0-a-minor-release-test.2
     * > [INFO  release_automation::lib::release] crates processed: 1, consistent: 1, published: 1, skipped: 1, tolerated: 0
     *
     * the above shouldn't have looked up .2 but rather .3, which it wouldn't have found
     */

    if !cmd_args.no_verify && !cmd_args.no_verify_post {
        info!("running consistency checks after changing the versions...");
        do_publish_to_crates_io(
            &selection,
            true,
            true,
            false,
            &cmd_args.allowed_missing_dependencies,
            &cmd_args.cargo_target_dir,
        )
        .context("cargo publish dry-run failed")?;
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
        ws.git_add_all_and_commit(&commit_msg, None)?;
    };

    if !cmd_args.no_tag_creation {
        // create tags for all released crates
        let tags_to_create = changed_crate_changelogs
            .iter()
            .map(|wcrh| wcrh.title())
            .collect::<Vec<String>>();
        create_crate_tags(ws, tags_to_create, cmd_args)?;
    }

    Ok(())
}

pub fn publish_to_crates_io<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    cmd_args: &'a ReleaseArgs,
) -> Fallible<()> {
    let crates = latest_release_crates(ws)?;

    do_publish_to_crates_io(
        &crates,
        cmd_args.dry_run,
        false,
        cmd_args.no_verify,
        &Default::default(),
        &cmd_args.cargo_target_dir,
    )?;

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
pub enum PublishError {
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

    #[error("{package}: check failed: {log}")]
    CheckFailure {
        package: String,
        version: String,
        log: String,
    },

    #[error("{}: {}", _0, _1)]
    Other(String, String),
}

impl PublishError {
    pub fn with_str(package: String, version: String, input: String) -> Self {
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
                // TODO FIXME
                #[allow(deprecated)]
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

/// Try to publish the given crates to crates.io.
///
/// If dry-run is given, the following error conditoins are tolerated:
/// - a dependency is not found but is part of the release
/// - a version of a dependency is not found bu the dependency is part of the release
///
/// For this to work properly all changed crates need to have their dev versions applied.
/// If they don't, `cargo publish` will prefer a published crates to the local ones.
pub fn do_publish_to_crates_io<'a>(
    crates: &[&'a Crate<'a>],
    dry_run: bool,
    allow_dirty: bool,
    no_verify: bool,
    allowed_missing_dependencies: &HashSet<String>,
    cargo_target_dir: &Option<PathBuf>,
) -> Fallible<()> {
    ensure_release_order_consistency(&crates).context("release ordering is broken")?;

    let crate_names: HashSet<String> = crates.iter().map(|crt| crt.name()).collect();

    debug!("attempting to publish {:?}", crate_names);

    let mut queue = crates.iter().collect::<std::collections::LinkedList<_>>();
    let mut errors: Vec<PublishError> = vec![];

    let mut check_cntr = 0;
    let mut publish_cntr = 0;
    let mut tolerated_cntr = 0;
    let mut skip_cntr = 0;
    let do_return =
        |errors: Vec<PublishError>, check_cntr, publish_cntr, skip_cntr, tolerated_cntr| {
            let msg = format!(
                "crates processed: {}, consistent: {}, published: {}, skipped: {}, tolerated: {}",
                crates.len(),
                check_cntr,
                publish_cntr,
                skip_cntr,
                tolerated_cntr,
            );

            info!("{}", msg);

            if !errors.is_empty() {
                let mut root = anyhow::anyhow!(msg);
                for error in errors.into_iter().rev() {
                    root = root.context(error);
                }
                Err(root)
            } else {
                Ok(())
            }
        };

    let mut published_or_tolerated = linked_hash_set::LinkedHashSet::new();

    let mut publish_cntr_inc = |name: &str| {
        info!("successfully published {}", name);
        publish_cntr += 1;
    };

    while let Some(crt) = queue.pop_front() {
        let state_changed = crt.state().changed();

        let name = crt.name().to_owned();
        let ver = crt.version().to_owned();

        let is_version_published = crates_index_helper::is_version_published(&name, &ver, false)?;

        if !state_changed && is_version_published {
            debug!(
                "{} is unchanged and already published, skipping..",
                crt.name_version()
            );
            skip_cntr += 1;
        }

        let manifest_path = crt.manifest_path();
        let cargo_target_dir_string = cargo_target_dir
            .as_ref()
            .map(|target_dir| format!("--target-dir={}", target_dir.to_string_lossy()));

        if !no_verify {
            let mut cmd = std::process::Command::new("cargo");
            cmd.args(
                [
                    vec![
                        "check",
                        "--locked",
                        "--verbose",
                        "--release",
                        &format!("--manifest-path={}", manifest_path.to_string_lossy()),
                    ],
                    if let Some(target_dir) = cargo_target_dir_string.as_ref() {
                        vec![target_dir]
                    } else {
                        vec![]
                    },
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

                let error = PublishError::CheckFailure {
                    package: crt.name(),
                    version: crt.version().to_string(),
                    log: details,
                };
                errors.push(error);
            } else {
                check_cntr += 1;
            }
        }

        let mut cmd = std::process::Command::new("cargo");
        cmd.args(
            [
                vec![
                    "publish",
                    "--locked",
                    "--verbose",
                    "--no-verify",
                    "--registry",
                    "crates-io",
                    &format!("--manifest-path={}", manifest_path.to_string_lossy()),
                ],
                if dry_run { vec!["--dry-run"] } else { vec![] },
                if allow_dirty {
                    vec!["--allow-dirty"]
                } else {
                    vec![]
                },
                if let Some(target_dir) = cargo_target_dir_string.as_ref() {
                    vec![target_dir]
                } else {
                    vec![]
                },
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
                    !((dry_run
                        && crate_names.contains(dependency)
                        && published_or_tolerated.contains(dependency))
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
                PublishError::CheckFailure { .. } => true,
            } {
                error!("{}", error);
                errors.push(error);
            } else {
                tolerated_cntr += 1;
                debug!("tolerating error: '{:#?}'", &error);

                published_or_tolerated.insert(crt.name());
            }
        } else if dry_run {
            publish_cntr_inc(&crt.name_version());
            published_or_tolerated.insert(crt.name());
        } else {
            // wait until the published version is live

            let mut found = false;

            for delay_secs in &[56, 28, 14, 7, 14, 28, 56] {
                let duration = std::time::Duration::from_secs(*delay_secs);
                std::thread::sleep(duration);

                if crates_index_helper::is_version_published(&crt.name(), &crt.version(), true)? {
                    debug!(
                        "Found recently published {} on crates.io!",
                        crt.name_version()
                    );
                    found = true;
                    break;
                }

                warn!(
                    "Did not find {} on crates.io, retrying in {:?}...",
                    crt.name_version(),
                    duration
                );
            }

            if !found {
                errors.push(PublishError::Other(
                    crt.name_version(),
                    "recently published version not found in time on the crates_io index"
                        .to_string(),
                ));

                return do_return(errors, check_cntr, publish_cntr, skip_cntr, tolerated_cntr);
            }

            publish_cntr_inc(&crt.name_version());
            published_or_tolerated.insert(crt.name());
        }
    }

    do_return(errors, check_cntr, publish_cntr, skip_cntr, tolerated_cntr)
}

/// create a tag for each crate which will be used to identify its latest release
fn create_crate_tags<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    tags_to_create: Vec<String>,
    cmd_args: &'a ReleaseArgs,
) -> Fallible<()> {
    let existing_tags = tags_to_create
        .iter()
        .filter_map(|git_tag| crate::crate_selection::git_lookup_tag(ws.git_repo(), git_tag))
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
pub fn ensure_release_branch<'a>(ws: &'a ReleaseWorkspace<'a>) -> Fallible<String> {
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
