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
