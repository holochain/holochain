use std::collections::{HashMap, HashSet};

use anyhow::bail;
use log::{debug, info, warn};
use semver::Version;
use structopt::StructOpt;

use crate::{crate_selection::Crate, release::ReleaseWorkspace, CommandResult, Fallible};

#[derive(StructOpt, Debug)]
pub(crate) struct CrateArgs {
    #[structopt(subcommand)]
    pub(crate) command: CrateCommands,
}

#[derive(Debug, StructOpt)]
pub(crate) struct CrateSetVersionArgs {
    #[structopt(long)]
    pub(crate) crate_name: String,

    #[structopt(long)]
    pub(crate) new_version: Version,
}

pub(crate) static DEFAULT_DEV_SUFFIX: &str = "dev.0";

#[derive(Debug, StructOpt)]
pub(crate) struct CrateApplyDevVersionsArgs {
    #[structopt(long, default_value = DEFAULT_DEV_SUFFIX)]
    pub(crate) dev_suffix: String,

    #[structopt(long)]
    pub(crate) dry_run: bool,

    #[structopt(long)]
    pub(crate) commit: bool,
}

#[derive(Debug)]
pub(crate) enum FixupReleases {
    Latest,
    All,
    Selected(Vec<String>),
}

/// Parses an input string to an ordered set of release steps.
pub(crate) fn parse_fixup_releases(input: &str) -> Fallible<FixupReleases> {
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
pub(crate) struct CrateFixupReleases {
    #[structopt(long, default_value = DEFAULT_DEV_SUFFIX)]
    pub(crate) dev_suffix: String,

    #[structopt(long)]
    pub(crate) dry_run: bool,

    #[structopt(long, default_value = "latest", parse(try_from_str = parse_fixup_releases))]
    pub(crate) fixup_releases: FixupReleases,

    #[structopt(long)]
    pub(crate) commit: bool,
}

#[derive(Debug, StructOpt)]
pub(crate) enum CrateCommands {
    SetVersion(CrateSetVersionArgs),
    ApplyDevVersions(CrateApplyDevVersionsArgs),

    /// check the latest (or given) release for crates that aren't published, remove their tags, and bump their version.
    FixupReleases(CrateFixupReleases),
}

pub(crate) fn cmd(args: &crate::cli::Args, cmd_args: &CrateArgs) -> CommandResult {
    let ws = ReleaseWorkspace::try_new(args.workspace_path.clone())?;

    match &cmd_args.command {
        CrateCommands::SetVersion(subcmd_args) => {
            let crt = *ws
                .members()?
                .iter()
                .find(|crt| crt.name() == subcmd_args.crate_name)
                .ok_or_else(|| anyhow::anyhow!("crate {} not found", subcmd_args.crate_name))?;

            crate::common::set_version(false, crt, &subcmd_args.new_version)?;

            Ok(())
        }

        CrateCommands::ApplyDevVersions(subcmd_args) => apply_dev_versions(
            &ws,
            &subcmd_args.dev_suffix,
            subcmd_args.dry_run,
            subcmd_args.commit,
        ),

        CrateCommands::FixupReleases(subcmd_args) => fixup_releases(
            &ws,
            &subcmd_args.dev_suffix,
            &subcmd_args.fixup_releases,
            subcmd_args.dry_run,
            subcmd_args.commit,
        ),
    }
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
pub(crate) fn apply_dev_versions<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    dev_suffix: &str,
    dry_run: bool,
    commit: bool,
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
            ws.cargo_check(false)?;

            if commit {
                ws.git_add_all_and_commit(&commit_msg, None)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn apply_dev_vesrions_to_selection<'a>(
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

        version.increment_patch();
        version = semver::Version::parse(&format!("{}-{}", version, dev_suffix))?;

        debug!(
            "[{}] rewriting version {} -> {}",
            crt.name(),
            crt.version(),
            version,
        );

        for changed_dependant in crate::common::set_version(dry_run, crt, &version)? {
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

pub(crate) fn fixup_releases<'a>(
    ws: &'a ReleaseWorkspace<'a>,
    dev_suffix: &str,
    fixup: &FixupReleases,
    dry_run: bool,
    commit: bool,
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
                if !crate::release::crates_index_helper::is_version_published(crt, false)? {
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
            ws.cargo_check(false)?;

            if commit {
                ws.git_add_all_and_commit(&commit_msg, None)?;
            }
        }
    }

    Ok(())
}
