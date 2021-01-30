#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use crate::error::HcBundleResult;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "hc-dna")]
/// Holochain DnaFile Utility.
pub enum HcDnaBundle {
    /// Bundle the contents of a directory into a `.dna` file, based on a
    /// `.dna.yaml` manifest file within that directory.
    ///
    /// e.g.: `hc-dna bundle some/directory/foo.dna.yaml` creates file `foo.dna`
    // #[structopt(short = "b", long)]
    Bundle {
        /// The path to the YAML manifest file
        manifest_path: std::path::PathBuf,
    },

    /// Unbundle the parts of `.dna` file out into a directory.
    ///
    /// (`hc-dna -u my-dna.dna` creates dir `my-dna`)
    // #[structopt(short = "u", long)]
    Unbundle {
        /// The path to the bundle to unpack
        bundle_path: std::path::PathBuf,
    },
}

impl HcDnaBundle {
    /// Run this command
    pub async fn run(self) -> HcBundleResult<()> {
        match self {
            Self::Bundle { manifest_path } => crate::dna::compress(&manifest_path, None).await,
            Self::Unbundle { bundle_path } => crate::dna::explode(&bundle_path, None).await,
        }
    }
}
