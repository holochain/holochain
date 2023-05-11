//! Helpers for working with DNA files.

use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::ensure;
use walkdir::WalkDir;

/// Parse a list of DNAs.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.dna`.
pub fn parse_dnas(mut dnas: Vec<PathBuf>) -> anyhow::Result<Vec<PathBuf>> {
    if dnas.is_empty() {
        dnas.push(std::env::current_dir()?);
    }
    for dna in dnas.iter_mut() {
        if dna.is_dir() {
            let file_path = search_for_dna(dna)?;
            *dna = file_path;
        }
        ensure!(
            dna.file_name()
                .map(|f| f.to_string_lossy().ends_with(".dna"))
                .unwrap_or(false),
            "File {} is not a valid dna file name: (e.g. my-dna.dna)",
            dna.display()
        );
    }
    Ok(dnas)
}

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

// TODO: Look for multiple dnas
fn search_for_dna(dna: &Path) -> anyhow::Result<PathBuf> {
    let dir: Vec<_> = WalkDir::new(dna)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_file())
        .filter(|f| f.file_name().to_string_lossy().ends_with(".dna"))
        .map(|f| f.into_path())
        .collect();
    if dir.len() != 1 {
        bail!(
            "Could not find a DNA file (e.g. my-dna.dna) in directory {}",
            dna.display()
        )
    }
    Ok(dir.into_iter().next().expect("Safe due to check above"))
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
