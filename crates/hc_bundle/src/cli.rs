#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use holochain_types::prelude::{AppManifest, DnaManifest};
use mr_bundle::Manifest;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::error::HcBundleResult;

/// The file extension to use for DNA bundles
pub const DNA_BUNDLE_EXT: &str = "dna";

/// The file extension to use for hApp bundles
pub const APP_BUNDLE_EXT: &str = "happ";

/// Work with Holochain DNA bundles
#[derive(Debug, StructOpt)]
pub enum HcDnaBundle {
    /// Create a new, empty Holochain DNA bundle working directory
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack the contents of a directory into a `.dna` bundle file.
    ///
    /// e.g.:
    ///
    /// $ hc-dna pack ./some/directory/foo/`
    ///
    /// would create file `./some/directory/foo/foo.dna`, if the `name` property in the dna.yaml file was `foo`
    Pack {
        /// The path to the unpacked directory containing a `dna.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the file will be placed inside the input directory,
        /// and given the name "[DNA_NAME].dna" where [DNA_NAME] is the `name` property of the `dna.yaml` file
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack the parts of `.dna` file out into a directory.
    ///
    /// (`hc-dna -u my-dna.dna` creates dir `my-dna`)
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked directory.
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Overwrite an existing directory, if one exists.
        #[structopt(short = "f", long)]
        force: bool,
    },
}

/// Work with Holochain hApp bundles
#[derive(Debug, StructOpt)]
pub enum HcAppBundle {
    /// Create a new, empty Holochain app (hApp) working directory
    Init {
        /// The path to create the directory
        path: PathBuf,
    },

    /// Pack the contents of a directory into a `.happ` bundle file.
    ///
    /// e.g.:
    ///
    /// $ hc-app pack ./some/directory/foo/`
    ///
    /// would create file `./some/directory/foo/foo.happ`, if the `name` property in the happ.yaml file was `foo`
    Pack {
        /// The path to the unpacked directory containing a `happ.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the file will be placed inside the input directory,
        /// and given the name "[HAPP_NAME].happ" where [HAPP_NAME] is the `name` property of the `happ.yaml` file
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack the parts of `.happ` file out into a directory.
    ///
    /// (`hc-app -u my-app.happ` creates dir `my-app`)
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked directory.
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Overwrite an existing directory, if one exists.
        #[structopt(short = "f", long)]
        force: bool,
    },
}

impl HcDnaBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_dna(path).await?;
            }
            Self::Pack { path, output } => {
                let name = get_dna_name(&path).await?;
                let (bundle_path, _) =
                    crate::packing::pack::<DnaManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                force,
            } => {
                let dir_path =
                    crate::packing::unpack::<DnaManifest>(DNA_BUNDLE_EXT, &path, output, force)
                        .await?;
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

impl HcAppBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_app(path).await?;
            }
            Self::Pack { path, output } => {
                let name = get_app_name(&path).await?;
                let (bundle_path, _) =
                    crate::packing::pack::<AppManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                force,
            } => {
                let dir_path =
                    crate::packing::unpack::<AppManifest>(APP_BUNDLE_EXT, &path, output, force)
                        .await?;
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

async fn get_dna_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&DnaManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: DnaManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.name())
}

async fn get_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&AppManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: AppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}
