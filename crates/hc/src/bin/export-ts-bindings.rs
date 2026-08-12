//! Writes the conductor API TypeScript binding tree.
//!
//! Driven by `make ts-bindings`. It is also meant to be buildable from a
//! consumer's own Nix flake, against the Holochain revision that flake pins,
//! so the consumer can regenerate the types it ships without taking them from
//! a release.
//!
//! The output directory comes from `TS_RS_EXPORT_DIR`, resolved relative to
//! the working directory. The TypeScript dialect is fixed here rather than
//! read from the environment: the generated tree targets holochain-client-js,
//! which needs plain `number` for 64-bit integers and `.js` extensions on
//! import paths, and those settings have to hold when the binary runs outside
//! a Cargo invocation of this workspace.
//!
//! # Building it from a consumer's flake
//!
//! Holonix exposes `packages.hc` as an overridable derivation whose build
//! arguments are `--manifest-path crates/hc/Cargo.toml --bin hc` with the
//! override appended, so a consumer picks this binary up by adding the feature
//! and the second binary:
//!
//! ```nix
//! inputs'.holonix.packages.hc.override {
//!   cargoExtraArgs = "--features ts_rs,unstable-countersigning --bin export-ts-bindings";
//! }
//! ```
//!
//! Putting the binary in a crate other than `hc` or `holochain` would break
//! this: the manifest path and the `--bin` are fixed in Holonix, and naming a
//! different package alongside them fails to resolve. Each `nix flake update`
//! on the consumer side moves the pinned revision forward, and re-running the
//! binary regenerates the types against it.

use std::path::{Path, PathBuf};

/// Removes its directory on drop, so a staging directory is cleaned up even
/// if the export fails before the happy path removes it.
struct StagingDir(PathBuf);

impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

/// Exports into a staging directory first, so the real output directory
/// (`TS_RS_EXPORT_DIR`, default `./bindings`) is never left in a half-written
/// state if the export fails partway through, and is only touched once the
/// full tree has exported successfully.
fn main() -> Result<(), ts_rs::ExportError> {
    let final_dir = ts_rs::Config::from_env().out_dir().to_path_buf();

    let staging = StagingDir(std::env::temp_dir().join(format!(
        "holochain-ts-bindings-{pid}",
        pid = std::process::id()
    )));
    let _ = std::fs::remove_dir_all(&staging.0);
    std::fs::create_dir_all(&staging.0)?;

    let cfg = ts_rs::Config::new()
        .with_large_int("number")
        .with_import_extension(Some("js"))
        .with_out_dir(staging.0.clone());
    holochain_conductor_api::export_ts_bindings(&cfg)?;

    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir)?;
    }
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    copy_dir_all(&staging.0, &final_dir)?;

    println!(
        "TypeScript bindings written to {path}",
        path = final_dir.display()
    );
    Ok(())
}
