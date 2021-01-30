#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use std::path::PathBuf;

use crate::error::HcBundleResult;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "hc-dna")]
/// Work with Holochain DNA bundle files
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

impl HcDnaBundle {
    /// Run this command
    pub async fn run(self) -> HcBundleResult<()> {
        match self {
            Self::Pack { path, output } => {
                let (bundle_path, _) = crate::dna::pack(&path, output).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                force,
            } => {
                let dir_path = crate::dna::unpack(&path, output, force).await?;
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}
