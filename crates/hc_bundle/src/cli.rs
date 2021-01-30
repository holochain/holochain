#![forbid(missing_docs)]
//! Binary `hc-dna-bundle` command executable.

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "hc-dna-bundle")]
/// Holochain DnaFile Utility.
pub struct HcDnaBundle {
    /// Bundle the contents of a directory into a `.dna` file.
    /// The inverse of `unbundle`.
    ///
    /// (`hc-dna -b my-dna/` creates file `my-dna.dna`)
    #[structopt(short = "b", long)]
    bundle: Option<std::path::PathBuf>,

    /// Unbundle the parts of `.dna` file out into a directory.
    /// The inverse of `bundle`.
    ///
    /// (`hc-dna -u my-dna.dna` creates dir `my-dna`)
    #[structopt(short = "u", long)]
    unbundle: Option<std::path::PathBuf>,
}
