use std::collections::{HashMap, HashSet};

use anyhow::bail;
use log::{debug, info, warn};
use semver::Version;
use structopt::StructOpt;

use crate::{release::ReleaseWorkspace, CommandResult, Fallible};

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

#[derive(Debug, StructOpt)]

pub(crate) struct CrateApplyDevVersionsArgs {
    #[structopt(long, default_value = "dev.0")]
    pub(crate) dev_suffix: String,

    #[structopt(long)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, StructOpt)]
pub(crate) enum CrateCommands {
    SetVersion(CrateSetVersionArgs),
    ApplyDevVersions(CrateApplyDevVersionsArgs),
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

        CrateCommands::ApplyDevVersions(subcmd_args) => {
            apply_dev_versions(&ws, &subcmd_args.dev_suffix, subcmd_args.dry_run)
        }
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
) -> Fallible<()> {
    let mut applicable_crates = ws
        .members()?
        .iter()
        .filter(|crt| crt.state().changed_since_previous_release())
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
            ws.cargo_check()?;

            ws.git_add_all_and_commit(&commit_msg, None)?;
        }
    }

    Ok(())
}
