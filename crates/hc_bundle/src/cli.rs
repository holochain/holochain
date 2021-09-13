#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use holochain_types::prelude::{AppManifest, DnaManifest};
use holochain_types::web_app::WebAppManifest;
use holochain_util::ffs;
use mr_bundle::Manifest;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::error::HcBundleResult;

/// The file extension to use for DNA bundles
pub const DNA_BUNDLE_EXT: &str = "dna";

/// The file extension to use for hApp bundles
pub const APP_BUNDLE_EXT: &str = "happ";

/// The file extension to use for Web-hApp bundles
pub const WEB_APP_BUNDLE_EXT: &str = "web-happ";

/// Work with Holochain DNA bundles
#[derive(Debug, StructOpt)]
pub enum HcDnaBundle {
    /// Create a new, empty Holochain DNA bundle working directory and create a new
    /// sample `dna.yaml` manifest inside.
    /// .
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].dna` bundle according to the `dna.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc dna pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].dna`, based on
    /// `./some/directory/foo/dna.yaml`
    Pack {
        /// The path to the working directory containing a `dna.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].dna` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack parts of the `.dna` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc dna unpack ./some/dir/my-dna.dna
    ///
    /// creates a new directory `./some/dir/my-dna`, containining a new `dna.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Overwrite an existing directory, if one exists
        #[structopt(short = "f", long)]
        force: bool,
    },
}

/// Work with Holochain hApp bundles
#[derive(Debug, StructOpt)]
pub enum HcAppBundle {
    /// Create a new, empty Holochain app (hApp) working directory and create a new
    /// sample `happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].happ` bundle according to the `happ.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc app pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].happ`, based on
    /// `./some/directory/foo/happ.yaml`
    Pack {
        /// The path to the working directory containing a `happ.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].happ` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack parts of the `.happ` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc app unpack ./some/dir/my-app.happ
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `happ.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Overwrite an existing directory, if one exists
        #[structopt(short = "f", long)]
        force: bool,
    },
}

/// Work with Holochain Web-hApp bundles
#[derive(Debug, StructOpt)]
pub enum HcWebAppBundle {
    /// Create a new, empty Holochain web app working directory and create a new
    /// sample `web-happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].web-happ` bundle according to the `web-happ.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc web-app pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].web-happ`, based on
    /// `./some/directory/foo/web-happ.yaml`
    Pack {
        /// The path to the working directory containing a `web-happ.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].web-happ` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack parts of the `.web-happ` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc web-app unpack ./some/dir/my-app.web-happ
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `web-happ.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Overwrite an existing directory, if one exists
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

impl HcWebAppBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_web_app(path).await?;
            }
            Self::Pack { path, output } => {
                let name = get_web_app_name(&path).await?;
                let (bundle_path, _) =
                    crate::packing::pack::<WebAppManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                force,
            } => {
                let dir_path = crate::packing::unpack::<WebAppManifest>(
                    WEB_APP_BUNDLE_EXT,
                    &path,
                    output,
                    force,
                )
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

async fn get_web_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&WebAppManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: WebAppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}
