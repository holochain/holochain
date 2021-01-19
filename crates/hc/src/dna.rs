//! Helpers for working with dna files.
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::anyhow;
use anyhow::ensure;
use structopt::StructOpt;
use tokio::process::Command;
use walkdir::WalkDir;

mod util;

#[derive(Debug, StructOpt)]
#[structopt(name = "dna-util")]
/// Holochain DnaFile Utility.
pub struct DnaUtil {
    /// Expand a DnaFile into a Dna Working Directory.
    ///
    /// (`dna-util -e my-dna.dna.gz` creates dir `my-dna.dna.workdir`)
    #[structopt(
        short = "e",
        long,
        required_unless = "compress",
        conflicts_with = "compress"
    )]
    pub expand: Option<std::path::PathBuf>,

    /// Compress a Dna Working Directory into a DnaFile.
    ///
    /// (`dna-util -c my-dna.dna.workdir` creates file `my-dna.dna.gz`)
    #[structopt(short = "c", long, required_unless = "expand")]
    pub compress: Option<std::path::PathBuf>,
}

/// Expand or compress dna.
pub async fn dna_util(opt: DnaUtil) -> util::DnaUtilResult<()> {
    if let Some(expand) = opt.expand {
        util::expand(&expand).await
    } else if let Some(compress) = opt.compress {
        util::compress(&compress).await
    } else {
        Ok(())
    }
}

/// Parse a list of dnas.
/// If paths are directories then each directory
/// will be searched for the first file that matches
/// `*.dna.gz`.
pub async fn parse_dnas(mut dnas: Vec<PathBuf>) -> anyhow::Result<Vec<PathBuf>> {
    if dnas.is_empty() {
        let current_dir = std::env::current_dir()?;
        if let Some(work_dir) = search_for_workdir(&current_dir) {
            try_compile(current_dir.clone()).await?;
            util::compress(&work_dir).await?;
        }
        dnas.push(current_dir);
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

fn search_for_workdir(path: &Path) -> Option<PathBuf> {
    WalkDir::new(path)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_dir())
        .find(|f| f.file_name().to_string_lossy().ends_with(".dna.workdir"))
        .map(|f| f.into_path())
}

async fn try_compile(mut path: PathBuf) -> anyhow::Result<()> {
    path.push("zomes");
    dbg!(&path);
    let zomes: Vec<_> = WalkDir::new(path)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_file())
        .filter(|f| f.file_name().to_string_lossy() == "Cargo.toml")
        .map(|f| f.into_path())
        .collect();
    for zome_path in zomes {
        let mut cmd = Command::new("cargo");
        if let Err(e) = cmd
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--manifest-path")
            .arg(&zome_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
        {
            tracing::error!("Failed to compile {:?}", e);
        } else {
            tracing::info!("Compiled {}", zome_path.display());
        }
    }

    Ok(())
}
