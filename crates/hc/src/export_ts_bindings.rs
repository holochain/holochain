//! The `hc export-ts-bindings` subcommand.
//!
//! Writes the conductor API TypeScript binding tree consumed by
//! holochain-client-js. It is built into `hc` behind the opt-in `ts_rs`
//! Cargo feature (off by default) so a consumer can regenerate the types
//! against the Holochain revision its own flake pins, by building `hc` with
//! `--features ts_rs`.
//!
//! The TypeScript dialect is fixed here rather than read from the
//! environment: the tree targets holochain-client-js, which needs plain
//! `number` for 64-bit integers and `.js` extensions on import paths, and
//! those settings must hold wherever the binary runs.
//!
//! Build `hc` with `--features unstable-countersigning` to include the
//! countersigning app API in the output.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context};
use clap::Parser;

/// Export the conductor API's TypeScript bindings.
#[derive(Debug, Parser)]
pub struct HcExportTsBindings {
    /// Directory the binding tree is written to.
    ///
    /// If it already exists, its contents are removed first. Must not be
    /// the current directory, one of its ancestors, or the root directory.
    #[arg(long, short = 'o', default_value = "bindings")]
    pub out_dir: PathBuf,
}

impl HcExportTsBindings {
    /// Run this command.
    pub fn run(self) -> anyhow::Result<()> {
        let final_dir = resolve_out_dir(&self.out_dir)?;
        refuse_dangerous_out_dir(&final_dir)?;

        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir)
                .with_context(|| format!("removing {}", final_dir.display()))?;
        }
        std::fs::create_dir_all(&final_dir)
            .with_context(|| format!("creating {}", final_dir.display()))?;

        let cfg = ts_rs::Config::new()
            .with_large_int("number")
            .with_import_extension(Some("js"))
            .with_out_dir(final_dir.clone());
        holochain_conductor_api::export_ts_bindings(&cfg)
            .context("exporting the TypeScript bindings")?;

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

/// Refuses to replace `final_dir` when it is the root directory, the current
/// directory, or one of the current directory's ancestors. This guard is
/// always on: there's no `--force` to bypass it.
fn refuse_dangerous_out_dir(final_dir: &Path) -> anyhow::Result<()> {
    if final_dir.parent().is_none() {
        bail!(
            "refusing to replace {}: it is the root directory",
            final_dir.display()
        );
    }
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

#[cfg(test)]
mod tests {
    use super::resolve_out_dir;

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
