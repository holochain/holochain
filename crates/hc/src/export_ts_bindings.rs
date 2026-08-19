//! The `hc export-ts-bindings` subcommand.
//!
//! Writes the conductor API TypeScript binding tree consumed by
//! holochain-client-js. It is a built-in subcommand of every `hc` build so a
//! consumer can regenerate the types against the Holochain revision its own
//! flake pins, using Holonix's `packages.hc` as is.
//!
//! The TypeScript dialect is fixed here rather than read from the
//! environment: the tree targets holochain-client-js, which needs plain
//! `number` for 64-bit integers and `.js` extensions on import paths, and
//! those settings must hold wherever the binary runs.
//!
//! Build `hc` with `--features unstable-countersigning` to include the
//! countersigning app API in the output.

use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use clap::Parser;

/// Export the conductor API's TypeScript bindings.
#[derive(Debug, Parser)]
pub struct HcExportTsBindings {
    /// Directory the binding tree is written to.
    ///
    /// Its previous contents are replaced by renaming a freshly exported
    /// tree into place: a failed export leaves it untouched, and if the
    /// swap itself fails partway through, the previous tree is restored on
    /// a best-effort basis. Must not be the current directory or one of its
    /// ancestors. If it already holds content that doesn't look like a
    /// previously generated binding tree (only directories and `.ts`
    /// files), the command refuses to replace it unless `--force` is given.
    #[arg(long, short = 'o', default_value = "bindings")]
    pub out_dir: PathBuf,

    /// Replace `out_dir` even if its contents don't look like a previously
    /// generated TypeScript binding tree.
    ///
    /// Never bypasses the refusal to replace the current directory or one
    /// of its ancestors; that guard is always on.
    #[arg(long, short = 'f')]
    pub force: bool,
}

impl HcExportTsBindings {
    /// Run this command.
    pub fn run(self) -> anyhow::Result<()> {
        let final_dir = resolve_out_dir(&self.out_dir)?;
        refuse_if_cwd_or_ancestor(&final_dir)?;
        refuse_unrecognized_contents(&final_dir, self.force)?;

        let parent = final_dir
            .parent()
            .ok_or_else(|| anyhow!("{} has no parent directory", final_dir.display()))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;

        // A sibling of `final_dir`, so the swap below is a same-filesystem
        // rename rather than a copy, and `tempdir_in` creates it securely
        // (no predictable path, no symlink-race window).
        let staging_dir = tempfile::Builder::new()
            .prefix(".hc-ts-bindings-")
            .tempdir_in(parent)
            .with_context(|| format!("creating a staging directory in {}", parent.display()))?;

        let cfg = ts_rs::Config::new()
            .with_large_int("number")
            .with_import_extension(Some("js"))
            .with_out_dir(staging_dir.path().to_path_buf());
        holochain_conductor_api::export_ts_bindings(&cfg)
            .context("exporting the TypeScript bindings")?;

        // Take ownership of the path so `TempDir::drop` doesn't try to clean
        // up a directory that `swap_into_place` is about to rename away.
        let staging_path = staging_dir.keep();
        swap_into_place(&staging_path, &final_dir)?;

        println!(
            "TypeScript bindings written to {path}",
            path = final_dir.display()
        );
        Ok(())
    }
}

/// Resolves `out_dir` to an absolute, canonical path for the guard checks.
///
/// `std::path::absolute` keeps `.` and `..` components, so `--out-dir ..`
/// would not compare equal to the parent directory; those are normalized
/// lexically first. If the resulting path exists it is canonicalized
/// outright (which also follows symlinks). Otherwise, the deepest existing
/// ancestor is canonicalized and the not-yet-existing tail is re-appended,
/// so a symlink in an intermediate path component is still resolved even
/// though the final component doesn't exist yet.
fn resolve_out_dir(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let abs =
        std::path::absolute(out_dir).with_context(|| format!("resolving {}", out_dir.display()))?;
    let mut normalized = PathBuf::new();
    for component in abs.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }

    if normalized.exists() {
        return normalized
            .canonicalize()
            .with_context(|| format!("resolving {}", out_dir.display()));
    }

    let mut tail = Vec::new();
    let mut existing = normalized.as_path();
    while !existing.exists() {
        let name = existing.file_name().with_context(|| {
            format!(
                "resolving {}: no existing ancestor directory",
                out_dir.display()
            )
        })?;
        tail.push(name.to_owned());
        existing = existing.parent().with_context(|| {
            format!(
                "resolving {}: no existing ancestor directory",
                out_dir.display()
            )
        })?;
    }
    let mut resolved = existing
        .canonicalize()
        .with_context(|| format!("resolving {}", out_dir.display()))?;
    for name in tail.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// Refuses to replace `final_dir` when it is the current directory or one of
/// its ancestors. This guard is always on, regardless of `--force`.
fn refuse_if_cwd_or_ancestor(final_dir: &Path) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()
        .and_then(|d| d.canonicalize())
        .context("reading the current directory")?;
    if cwd.starts_with(final_dir) {
        bail!(
            "refusing to replace {}: it is the current directory or one of its ancestors",
            final_dir.display()
        );
    }
    Ok(())
}

/// Refuses to replace `final_dir` if it holds content that doesn't look like
/// a previously generated binding tree, unless `force` is set. This is a
/// content-safety check, layered on top of (but distinct from) the
/// cwd/ancestor guard.
fn refuse_unrecognized_contents(final_dir: &Path, force: bool) -> anyhow::Result<()> {
    if force || !final_dir.exists() {
        return Ok(());
    }
    if looks_like_binding_tree(final_dir)
        .with_context(|| format!("reading {}", final_dir.display()))?
    {
        return Ok(());
    }
    bail!(
        "refusing to replace {}: it does not look like a previously generated TypeScript \
         binding tree (expected only directories and files ending in \".ts\"); pass --force \
         to replace it anyway",
        final_dir.display()
    );
}

/// Returns whether every top-level entry of `dir` is a directory or a file
/// whose name ends in `.ts`, i.e. whether `dir` looks like a tree this
/// command generated, and so is safe to replace without `--force`.
fn looks_like_binding_tree(dir: &Path) -> std::io::Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(".ts"))
        {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

/// Replaces `final_dir`'s contents with the freshly exported tree at
/// `staging_path`, which must be a sibling of `final_dir` (i.e. on the same
/// filesystem) so the swap is a rename rather than a copy.
///
/// If `final_dir` already exists, it is renamed aside first and only
/// removed once the new tree is safely in place, so a failure partway
/// through leaves the on-disk outcome as either "old tree intact" or "new
/// tree in place" -- never neither.
fn swap_into_place(staging_path: &Path, final_dir: &Path) -> anyhow::Result<()> {
    if !final_dir.exists() {
        return std::fs::rename(staging_path, final_dir)
            .with_context(|| format!("writing {}", final_dir.display()));
    }

    let trash = PathBuf::from(format!(
        "{final_dir}.hc-ts-bindings-trash",
        final_dir = final_dir.display()
    ));
    let _ = std::fs::remove_dir_all(&trash);
    std::fs::rename(final_dir, &trash)
        .with_context(|| format!("moving aside the previous {}", final_dir.display()))?;

    if let Err(err) = std::fs::rename(staging_path, final_dir) {
        // Best-effort restore: the on-disk outcome must be either "old tree
        // intact" or "new tree in place", never neither.
        let _ = std::fs::rename(&trash, final_dir);
        return Err(err).with_context(|| format!("writing {}", final_dir.display()));
    }

    let _ = std::fs::remove_dir_all(&trash);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{resolve_out_dir, swap_into_place};

    #[test]
    fn swap_into_place_restores_the_previous_tree_if_the_final_rename_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let final_dir = tmp.path().join("final");
        std::fs::create_dir_all(&final_dir).unwrap();
        std::fs::write(final_dir.join("keep.ts"), "export type Keep = never;\n").unwrap();

        // A staging path that doesn't exist forces the second rename
        // (staging into final_dir) to fail after the first rename
        // (final_dir moved aside to trash) has already succeeded.
        let bogus_staging = tmp.path().join("does-not-exist");

        let result = swap_into_place(&bogus_staging, &final_dir);

        assert!(result.is_err());
        assert!(
            final_dir.join("keep.ts").exists(),
            "the previous tree must be restored when the swap fails partway through"
        );
    }

    #[test]
    fn resolve_out_dir_canonicalizes_an_existing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        let resolved = resolve_out_dir(&nested).unwrap();

        assert_eq!(resolved, nested.canonicalize().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_out_dir_follows_a_symlink_before_a_not_yet_existing_component() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        // `link/not-yet-created` doesn't exist, but `link` -> `real` does;
        // the resolved path must land under the symlink's target, not
        // under a literal `tmp/link/not-yet-created`.
        let resolved = resolve_out_dir(&link.join("not-yet-created")).unwrap();

        assert_eq!(
            resolved,
            real_dir.canonicalize().unwrap().join("not-yet-created")
        );
    }
}
