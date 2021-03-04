//! Helpers for working with dna files.
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::ensure;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
/// Either a set of Dnas or a hApp bundle.
pub enum DnasHapp {
    /// Set of Dnas to install.
    Dnas(Vec<PathBuf>),
    /// Happ bundle to install.
    HApp(Option<PathBuf>),
}

impl DnasHapp {
    /// Create a new DnasHapp from either dnas or happ bundle.
    pub fn new(dnas: Option<Vec<PathBuf>>, happ: Option<Option<PathBuf>>) -> Self {
        match (dnas, happ) {
            (Some(dnas), None) => Self::Dnas(dnas),
            (None, Some(happ)) => Self::HApp(happ),
            _ => unreachable!("Cannot have both dnas and happ"),
        }
    }

    /// Parse the underlying paths.
    pub fn parse(self) -> anyhow::Result<Self> {
        match self {
            Self::Dnas(dnas) => {
                let dnas = parse_dnas(dnas)?;
                Ok(Self::Dnas(dnas))
            }
            Self::HApp(happ) => {
                let happ = parse_happ(happ)?;
                Ok(Self::HApp(Some(happ)))
            }
        }
    }
}

/// Parse a list of dnas.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.dna`.
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
                .map(|f| f.to_string_lossy().ends_with(".dna"))
                .unwrap_or(false),
            "File {} is not a valid dna file name: (e.g. my-dna.dna)",
            dna.display()
        );
    }
    Ok(dnas)
}

/// Parse a happ bundle.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.dna`.
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
        "File {} is not a valid dna file name: (e.g. my-dna.dna)",
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
                "Could not find a DNA file (e.g. my-dna.dna) in directory {}",
                happ.display()
            )
        }
    }
}
