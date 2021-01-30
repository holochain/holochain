#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use crate::error::HcBundleResult;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "hc-dna")]
/// Work with Holochain DNA bundle files
pub enum HcDnaBundle {
    /// Pack the contents of a directory into a `.dna` bundle file, based on a
    /// `.dna.yaml` manifest file within that directory.
    ///
    /// e.g.: `hc-dna pack some/directory/foo.dna.yaml` creates file `foo.dna`
    Pack {
        /// The path to the YAML manifest file
        manifest_path: std::path::PathBuf,
    },

    /// Unpack the parts of `.dna` file out into a directory.
    ///
    /// (`hc-dna -u my-dna.dna` creates dir `my-dna`)
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        bundle_path: std::path::PathBuf,
    },
}

impl HcDnaBundle {
    /// Run this command
    pub async fn run(self) -> HcBundleResult<()> {
        match self {
            Self::Pack { manifest_path } => crate::dna::compress(&manifest_path, None).await,
            Self::Unpack { bundle_path } => crate::dna::unpack(&bundle_path, None).await,
        }
    }
}
