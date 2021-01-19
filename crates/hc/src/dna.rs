//! Helpers for working with dna files.
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::ensure;
use walkdir::WalkDir;

/// Parse a list of dnas.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.dna.gz`.
pub fn parse_dnas(mut dnas: Vec<PathBuf>) -> anyhow::Result<Vec<PathBuf>> {
    if dnas.is_empty() {
        dnas.push(std::env::current_dir()?);
    }
    for dna in dnas.iter_mut() {
        if dna.is_dir() {
            let file_path = search_for_dna(&dna)?;
            *dna = file_path;
        }
        ensure!(
            dna.file_name()
                .map(|f| f.to_string_lossy().ends_with(".dna.gz"))
                .unwrap_or(false),
            "File {} is not a valid dna file name: (e.g. my-dna.dna.gz)",
            dna.display()
        );
    }
    Ok(dnas)
}

fn search_for_dna(dna: &Path) -> anyhow::Result<PathBuf> {
    let dir = WalkDir::new(dna)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_file())
        .find(|f| f.file_name().to_string_lossy().ends_with(".dna.gz"))
        .map(|f| f.into_path());
    dir.ok_or_else(|| {
        anyhow!(
            "Could not find a dna (e.g. my-dna.dna.gz) in directory {}",
            dna.display()
        )
    })
}
