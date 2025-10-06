//! Helpers for working with DNA files.

use anyhow::bail;
use anyhow::ensure;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Parse a hApp bundle.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.happ`.
pub fn parse_happ(happ: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let mut happ = happ.unwrap_or(std::env::current_dir()?);
    if happ.is_dir() {
        let file_path = search_for_happ(&happ)?;
        happ = file_path;
    }
    ensure!(
        happ.file_name()
            .map(|f| f.to_string_lossy().ends_with(".happ"))
            .unwrap_or(false),
        "File {} is not a valid happ file name: (e.g. my-happ.happ)",
        happ.display()
    );
    Ok(happ)
}

fn search_for_happ(happ: &Path) -> anyhow::Result<PathBuf> {
    let dir = WalkDir::new(happ)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_file())
        .find(|f| f.file_name().to_string_lossy().ends_with(".happ"))
        .map(|f| f.into_path());
    match dir {
        Some(dir) => Ok(dir),
        None => {
            bail!(
                "Could not find a happ file (e.g. my-happ.happ) in directory {}",
                happ.display()
            )
        }
    }
}
