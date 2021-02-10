#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use std::path::PathBuf;

use crate::error::HcBundleResult;
use holochain_types::prelude::{AppManifest, DnaManifest};
use structopt::StructOpt;

/// The file extension to use for DNA bundles
pub const DNA_BUNDLE_EXT: &str = "dna";

/// The file extension to use for hApp bundles
pub const APP_BUNDLE_EXT: &str = "happ";

/// Work with Holochain DNA bundles
#[derive(Debug, StructOpt)]
pub enum HcDnaBundle {
    /// Pack the contents of a directory into a `.dna` bundle file.
    ///
    /// e.g.:
    ///
    /// $ hc-dna pack ./some/directory/foo/`
    ///
    /// will create file `./some/directory/foo.dna`
    Pack {
        /// The path to the unpacked directory containing a `dna.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the file will be placed alongside the input directory,
        /// and given the name "[DIRECTORY].dna"
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
    /// Pack the contents of a directory into a `.happ` bundle file.
    ///
    /// e.g.:
    ///
    /// $ hc-app pack ./some/directory/foo/`
    ///
    /// will create file `./some/directory/foo.happ`
    Pack {
        /// The path to the unpacked directory containing a `app.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the file will be placed alongside the input directory,
        /// and given the name "[DIRECTORY].happ"
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
    pub async fn run(self) -> HcBundleResult<()> {
        match self {
            Self::Pack { path, output } => {
                let (bundle_path, _) = crate::packing::pack::<DnaManifest>(&path, output).await?;
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
    pub async fn run(self) -> HcBundleResult<()> {
        match self {
            Self::Pack { path, output } => {
                let (bundle_path, _) = crate::packing::pack::<AppManifest>(&path, output).await?;
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
