#![deny(missing_docs)]
//! Binary `dna_util` command executable.

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dna_util", about = "Holochain DnaFile Utility.")]
struct Opt {
    /// Extract a DnaFile into a Dna Working Directory
    #[structopt(short = "e", long)]
    extract: Option<std::path::PathBuf>,

    /// Compile a Dna Working Directory into a DnaFile
    #[structopt(short = "c", long)]
    compile: Option<std::path::PathBuf>,
}

/// Main `dna_util` executable entrypoint.
#[tokio::main(threaded_scheduler)]
pub async fn main() {
    let opt = Opt::from_args();

    if opt.extract.is_none() && opt.compile.is_none() {
        Opt::clap().print_help().unwrap();
        return;
    }

    println!("yo");
}
