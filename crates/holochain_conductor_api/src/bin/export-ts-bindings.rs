//! Writes the conductor API TypeScript binding tree.
//!
//! Driven by `make ts-bindings`. The output directory and the TypeScript
//! dialect come from the `TS_RS_*` environment variables read by
//! [`ts_rs::Config::from_env`]; the repo's defaults for the latter live in
//! `.cargo/config.toml`. `TS_RS_EXPORT_DIR` is resolved relative to the
//! working directory.

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

    let staging = StagingDir(
        std::env::temp_dir().join(format!("holochain-ts-bindings-{}", std::process::id())),
    );
    let _ = std::fs::remove_dir_all(&staging.0);
    std::fs::create_dir_all(&staging.0)?;

    let cfg = ts_rs::Config::from_env().with_out_dir(staging.0.clone());
    holochain_conductor_api::export_ts_bindings(&cfg)?;

    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir)?;
    }
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    copy_dir_all(&staging.0, &final_dir)?;

    println!("TypeScript bindings written to {}", final_dir.display());
    Ok(())
}
