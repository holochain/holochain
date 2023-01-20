use anyhow::{bail, Context};
use bstr::ByteSlice;
use cargo::util::VersionExt;
use linked_hash_map::LinkedHashMap;
use linked_hash_set::LinkedHashSet;
use log::{debug, info, trace, warn};
use semver::{Comparator, Version, VersionReq};
use std::collections::{HashMap, HashSet};
use structopt::StructOpt;

use crate::{
    common::{increment_semver, SemverIncrementMode},
    crate_selection::Crate,
    release::ReleaseWorkspace,
    CommandResult, Fallible,
};

#[derive(StructOpt, Debug)]
pub struct CrateArgs {
    #[structopt(subcommand)]
    pub command: CrateCommands,
}

#[derive(Debug, StructOpt)]
pub struct CrateSetVersionArgs {
    #[structopt(long)]
    pub crate_name: String,

    #[structopt(long)]
    pub new_version: Version,
}

pub static DEFAULT_DEV_SUFFIX: &str = "dev.0";

#[derive(Debug, StructOpt)]
pub struct CrateApplyDevVersionsArgs {
    #[structopt(long, default_value = DEFAULT_DEV_SUFFIX)]
    pub dev_suffix: String,

    #[structopt(long)]
    pub dry_run: bool,

    #[structopt(long)]
    pub commit: bool,

    #[structopt(long)]
    pub no_verify: bool,
}

#[derive(Debug)]
pub enum FixupReleases {
    Latest,
    All,
    Selected(Vec<String>),
}

/// Parses an input string to an ordered set of release steps.
pub fn parse_fixup_releases(input: &str) -> Fallible<FixupReleases> {
    use std::str::FromStr;

    let words = input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>();

    if let Some(first) = words.first() {
        match first.as_str() {
            "latest" => return Ok(FixupReleases::Latest),
            "all" => return Ok(FixupReleases::All),
            _ => {}
        }
    }

    Ok(FixupReleases::Selected(words))
}

#[derive(Debug, StructOpt)]
pub struct CrateFixupUnpublishedReleases {
    #[structopt(long, default_value = DEFAULT_DEV_SUFFIX)]
    pub dev_suffix: String,

    #[structopt(long)]
    pub dry_run: bool,

    #[structopt(long, default_value = "latest", parse(try_from_str = parse_fixup_releases))]
    pub fixup_releases: FixupReleases,

    #[structopt(long)]
    pub commit: bool,

    #[structopt(long)]
    pub no_verify: bool,
}

#[derive(Debug, StructOpt)]
pub struct CrateDetectMissingReleaseheadings {}

#[derive(Debug, StructOpt)]
pub struct CrateCheckArgs {
    #[structopt(long)]
    offline: bool,
}

/// These crate.io handles are used as the default minimum crate owners for all published crates.
pub const MINIMUM_CRATE_OWNERS: &str =
    "github:holochain:core-dev,holochain-release-automation,holochain-release-automation2,zippy,steveeJ";

#[derive(Debug, StructOpt)]
pub struct EnsureCrateOwnersArgs {
    #[structopt(long)]
    dry_run: bool,

    /// Assumes the default crate owners that are ensured to be set for each crate in the workspace.
    #[structopt(
        long,
        default_value = MINIMUM_CRATE_OWNERS,
        use_delimiter = true,
        multiple = false,

    )]
    minimum_crate_owners: Vec<String>,
}

#[derive(Debug, StructOpt)]
pub struct CratePinDepsArgs {
    #[structopt(long)]
    dry_run: bool,

    #[structopt(long, default_value = "=")]
    version_prefix: String,

    crt: String,
}

#[derive(Debug, StructOpt)]
pub struct CrateMakePinnedArgs {
    #[structopt(long)]
    dry_run: bool,

    #[structopt(long, default_value = "=")]
    version_prefix: String,

    crt: String,
}

#[derive(Debug, StructOpt)]
pub enum CrateCommands {
    SetVersion(CrateSetVersionArgs),
    ApplyDevVersions(CrateApplyDevVersionsArgs),

    /// check the latest (or given) release for crates that aren't published, remove their tags, and bump their version.
    FixupUnpublishedReleases(CrateFixupUnpublishedReleases),

    /// verify that all published crates have a heading in their changelog
    DetectMissingReleaseheadings(CrateDetectMissingReleaseheadings),

    Check(CrateCheckArgs),
    EnsureCrateOwners(EnsureCrateOwnersArgs),

    /// Pins all dependencies of a given crate and its path dependencies recursively
    PinDeps(CratePinDepsArgs),

    /// Makes a given crate a pinned dependency in the entire workspace
    MakePinnedDep(CrateMakePinnedArgs),
}

pub fn cmd(args: &crate::cli::Args, cmd_args: &CrateArgs) -> CommandResult {
    let ws = ReleaseWorkspace::try_new(args.workspace_path.clone())?;

    match &cmd_args.command {
        CrateCommands::SetVersion(subcmd_args) => {
            let crt = *ws
                .members()?
                .iter()
                .find(|crt| crt.name() == subcmd_args.crate_name)
                .ok_or_else(|| anyhow::anyhow!("crate {} not found", subcmd_args.crate_name))?;

            crt.set_version(false, &subcmd_args.new_version)?;

            Ok(())
        }

        CrateCommands::ApplyDevVersions(subcmd_args) => apply_dev_versions(
            &ws,
            &subcmd_args.dev_suffix,
            subcmd_args.dry_run,
            subcmd_args.commit,
            subcmd_args.no_verify,
        ),

        CrateCommands::FixupUnpublishedReleases(subcmd_args) => fixup_unpublished_releases(
            &ws,
            &subcmd_args.dev_suffix,
            &subcmd_args.fixup_releases,
            subcmd_args.dry_run,
            subcmd_args.commit,
            subcmd_args.no_verify,
        ),

        CrateCommands::Check(subcmd_args) => {
            ws.cargo_check(subcmd_args.offline, std::iter::empty::<&str>())?;

            Ok(())
        }
        CrateCommands::EnsureCrateOwners(subcmd_args) => {
            ensure_crate_io_owners(
                &ws,
                subcmd_args.dry_run,
                ws.members()?,
                subcmd_args.minimum_crate_owners.as_slice(),
            )?;

            Ok(())
        }
        CrateCommands::DetectMissingReleaseheadings(subcmd_args) => {
            cmd_detect_missing_releaseheadings(&ws, subcmd_args)
        }
        CrateCommands::PinDeps(subcmd_args) => pin_deps(&ws, subcmd_args),
        CrateCommands::MakePinnedDep(subcmd_args) => make_pinned_dep(&ws, subcmd_args),
    }
}

fn pin_deps<'a>(
    _ws: &'a ReleaseWorkspace<'a>,
    _subcmd_args: &CratePinDepsArgs,
) -> Result<(), anyhow::Error> {
    todo!()
}

fn make_pinned_dep<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    subcmd_args: &CrateMakePinnedArgs,
) -> Result<(), anyhow::Error> {
    let crt = ws
        .members()?
        .into_iter()
        .find(|member| member.name() == subcmd_args.crt)
        .ok_or(anyhow::anyhow!(
            "looking for crate {} in workspace",
            subcmd_args.crt
        ))?;

    for dependant in crt.dependants_in_workspace()? {
        dependant.set_dependency_version(
            &crt.name(),
            &crt.version(),
            Some(&semver::VersionReq::parse(&format!(
                "{}{}",
                subcmd_args.version_prefix,
                crt.version()
            ))?),
            subcmd_args.dry_run,
        )?;
    }

    Ok(())
}

fn cmd_detect_missing_releaseheadings<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    _subcmd_args: &CrateDetectMissingReleaseheadings,
) -> Fallible<()> {
    let missing_headings = detect_missing_releaseheadings(ws)?;

    if !missing_headings.is_empty() {
        bail!("missing crate release headings: {:#?}", missing_headings);
    }

    Ok(())
}

/// if there are any crate release headings present in the workspace changelog but missing from the crate changelogs an error is returned.
///
/// uses the workspace changelog as a source of truth for existing crate releases.
/// this reasonable because the workspace changelog it's only changed on releases it's not prone to manual mistakes.
pub fn detect_missing_releaseheadings<'a>(
    ws: &'a ReleaseWorkspace<'a>,
) -> Fallible<LinkedHashMap<String, LinkedHashSet<String>>> {
    use itertools::Itertools;

    let crate_headings_toplevel = {
        let cl = ws
            .changelog()
            // .map(|cl| cl.topmost_release())
            .ok_or_else(|| {
                anyhow::anyhow!("no changelog found in workspace at '{:?}'", ws.root())
            })?;

        cl.changes()?
            .iter()
            .map(|change| match change {
                crate::changelog::ChangeT::Release(rc) => match rc {
                    crate::changelog::ReleaseChange::WorkspaceReleaseChange(_title, releases) => {
                        Ok(releases.clone())
                    }
                    unexpected => bail!("expected a WorkspaceReleaseChange here: {:?}", unexpected),
                },
                _ => Ok(vec![]),
            })
            // TODO: what happens with errors here?
            .flatten_ok()
            .collect::<Fallible<Vec<_>>>()?
    };

    let crate_headings_toplevel_by_crate = crate_headings_toplevel.into_iter().try_fold(
        LinkedHashMap::<String, LinkedHashSet<String>>::new(),
        |mut acc, cur| -> Fallible<_> {
            // TODO: use crate names to detect the split instead of the delimiter
            let (crt, version) = cur.split_once('-').ok_or(anyhow::anyhow!(
                "could not split '{}' by
                '-'",
                cur
            ))?;

            acc.entry(crt.to_string())
                .or_insert_with(|| Default::default())
                .insert(version.to_string());

            Ok(acc)
        },
    )?;

    trace!(
        "toplevel crate headings: {:#?}",
        crate_headings_toplevel_by_crate
    );

    let crate_headings_cratedirs = ws.members()?.into_iter().try_fold(
        LinkedHashMap::<String, LinkedHashSet<String>>::new(),
        |mut acc, crt| -> Fallible<_> {
            let name = crt.name();

            let crt_released_versions = if let Some(cl) = crt.changelog() {
                cl.changes()?
                    .iter()
                    .map(|change| match change {
                        crate::changelog::ChangeT::Release(rc) => match rc {
                            crate::changelog::ReleaseChange::CrateReleaseChange(version) => {
                                Ok(Some(version.clone()))
                            }
                            unexpected => {
                                bail!("expected a CrateReleaseChange here: {:?}", unexpected)
                            }
                        },
                        _ => Ok(None),
                    })
                    // TODO: what happens with errors here?
                    .flatten_ok()
                    .collect::<Fallible<Vec<_>>>()?
            } else {
                // don't change the result if the crate has no changelog
                return Ok(acc);
            };

            acc.entry(name)
                .or_insert_with(|| Default::default())
                .extend(crt_released_versions);

            Ok(acc)
        },
    )?;

    let missing_headings: LinkedHashMap<String, LinkedHashSet<String>> =
        crate_headings_toplevel_by_crate
            .iter()
            .filter_map(|(crt, headings)| {
                // only consider crates that still exist as they could have been deleted entirely at some point
                if let Some(headings_crate) = crate_headings_cratedirs.get(crt) {
                    let diff = headings
                        .difference(headings_crate)
                        .cloned()
                        .collect::<LinkedHashSet<_>>();

                    if !diff.is_empty() {
                        Some((crt.to_owned(), diff))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

    Ok(missing_headings)
}

/// Scans the workspace for crates that have changed since their previous release and bumps their version to a dev version.
///
/// This is a crucial part of the release flow to prevent inconsistencies in publishing dependents of these changed crates.
/// For example:
/// crate A is being published and depends on crate B and its changes since its last release.
/// crate B however hasn't increased its version number since the last release, so it looks as if the most recent version is already published.
/// This causes crate A to be published with a dependency on version of crate B that doesn't contain the changes that crate A depends upon.
/// Hence the newly published version of crate A is broken.
/// To prevent this, we increase crate B's version to a develop version that hasn't been published yet.
/// This will detect a missing dependency in an attempt to publish crate A, as the dev version of crate B is not found on the registry.
/// Note that we wouldn't publish the develop version of crate B, as the regular workspace release flow also increases its version according to the configured scheme.
pub fn apply_dev_versions<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    dev_suffix: &str,
    dry_run: bool,
    commit: bool,
    no_verify: bool,
) -> Fallible<()> {
    let applicable_crates = ws
        .members()?
        .iter()
        .filter(|crt| crt.state().changed_since_previous_release())
        .cloned()
        .collect::<Vec<_>>();

    let msg = apply_dev_vesrions_to_selection(applicable_crates, dev_suffix, dry_run)?;

    if !msg.is_empty() {
        let commit_msg = indoc::formatdoc! {r#"
            apply develop versions to changed crates

            the following crates changed since their most recent release
            and are therefore increased to a develop version:
            {}
        "#, msg,
        };

        info!("creating commit with message '{}' ", commit_msg);

        if !dry_run {
            // this checks consistency and also updates the Cargo.lock file(s)
            if !no_verify {
                ws.cargo_check(false, std::iter::empty::<&str>())?;
            }

            if commit {
                ws.git_add_all_and_commit(&commit_msg, None)?;
            }
        }
    }

    Ok(())
}

pub fn apply_dev_vesrions_to_selection<'a>(
    applicable_crates: Vec<&'a Crate<'a>>,
    dev_suffix: &str,
    dry_run: bool,
) -> Fallible<String> {
    let mut applicable_crates = applicable_crates
        .iter()
        .map(|crt| (crt.name(), *crt))
        .collect::<HashMap<_, _>>();

    let mut queue = applicable_crates.values().copied().collect::<Vec<_>>();
    let mut msg = String::new();

    while let Some(crt) = queue.pop() {
        let mut version = crt.version();

        if version.is_prerelease() {
            debug!(
                "[{}] ignoring due to prerelease version '{}' after supposed release",
                crt.name(),
                version,
            );

            continue;
        }

        increment_semver(&mut version, SemverIncrementMode::Patch)?;
        version = semver::Version::parse(&format!("{}-{}", version, dev_suffix))?;

        debug!(
            "[{}] rewriting version {} -> {}",
            crt.name(),
            crt.version(),
            version,
        );

        for changed_dependant in crt.set_version(dry_run, &version)? {
            if applicable_crates
                .insert(changed_dependant.name(), changed_dependant)
                .is_none()
                && changed_dependant.state().has_previous_release()
            {
                queue.push(changed_dependant);
            }
        }

        // todo: can we mutate crt and use crt.name_version() here instead?
        msg += format!("\n- {}-{}", crt.name(), version).as_str();
    }

    Ok(msg)
}

pub fn fixup_unpublished_releases<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    dev_suffix: &str,
    fixup: &FixupReleases,
    dry_run: bool,
    commit: bool,
    no_verify: bool,
) -> Fallible<()> {
    let mut unpublished_crates: std::collections::BTreeMap<
        String,
        Vec<&'a crate::crate_selection::Crate>,
    > = Default::default();

    match fixup {
        FixupReleases::Latest => {
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
                crate::changelog::ReleaseChange::WorkspaceReleaseChange(title, releases) => (
                    title,
                    releases
                        .into_iter()
                        .collect::<std::collections::BTreeSet<_>>(),
                ),
                unexpected => bail!("unexpected topmost release: {:?}", unexpected),
            };

            debug!("{}: {:#?}", release_title, crate_release_titles);

            let crates = ws
                .members()?
                .iter()
                .filter(|crt| crate_release_titles.contains(&crt.name_version()))
                .cloned()
                .collect::<Vec<_>>();

            for crt in crates {
                if !crates_index_helper::is_version_published(&crt.name(), &crt.version(), false)? {
                    unpublished_crates
                        .entry(release_title.clone())
                        .or_default()
                        .push(crt);
                }
            }
        }
        other => bail!("{:?} not implemented", other),
    }

    info!(
        "the following crates are unpublished: {:#?}",
        unpublished_crates
            .iter()
            .map(|(release, crts)| (
                release,
                crts.iter()
                    .map(|crt| crt.name_version())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );

    // bump their versions to dev versions
    let msg = apply_dev_vesrions_to_selection(
        // TOOD: change this once more than "latest" is supported above
        unpublished_crates.into_iter().next().unwrap_or_default().1,
        dev_suffix,
        dry_run,
    )?;

    if !msg.is_empty() {
        let commit_msg = indoc::formatdoc! {r#"
            applying develop versions to unpublished crates

            bumping the following crates to their dev versions to retrigger the release process for the failed crates
            {}
        "#, msg,
        };

        info!("creating commit with message '{}' ", commit_msg);

        if !dry_run {
            // this checks consistency and also updates the Cargo.lock file(s)
            if !no_verify {
                ws.cargo_check(false, std::iter::empty::<&str>())?;
            };

            if commit {
                ws.git_add_all_and_commit(&commit_msg, None)?;
            }
        }
    }

    Ok(())
}

/// Ensures that the given crates have at least sent an invite to the given crate.io usernames.
pub fn ensure_crate_io_owners<'a>(
    _ws: &'a ReleaseWorkspace<'a>,
    dry_run: bool,
    crates: &[&Crate],
    minimum_crate_owners: &[String],
) -> Fallible<()> {
    let desired_owners = minimum_crate_owners
        .into_iter()
        .cloned()
        .collect::<HashSet<String>>();

    for crt in crates {
        if !crates_index_helper::is_version_published(&crt.name(), &crt.version(), false)? {
            warn!("{} is not published, skipping..", crt.name());
            continue;
        }

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
            if !dry_run {
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

    Ok(())
}
