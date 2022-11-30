//! Check command functionality.

use super::*;

/// Parses the workspace for release candidates and checks for blocking conditions.
pub(crate) fn cmd(args: &cli::Args, cmd_args: &cli::CheckArgs) -> CommandResult {
    let ws = crate_selection::ReleaseWorkspace::try_new_with_criteria(
        args.workspace_path.clone(),
        cmd_args.to_selection_criteria(&args),
    )?;

    let release_candidates = common::selection_check(cmd_args, &ws)?;

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
