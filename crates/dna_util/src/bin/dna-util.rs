#![forbid(missing_docs)]
//! Binary `dna_util` command executable.

use dna_util::DnaUtilResult;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dna-util")]
/// Holochain DnaFile Utility.
struct Opt {
    /// Expand a DnaFile into a Dna Working Directory.
    ///
    /// (`dna-util -e my-dna.dna.gz` creates dir `my-dna.dna.workdir`)
    #[structopt(short = "e", long)]
    expand: Option<std::path::PathBuf>,

    /// Compress a Dna Working Directory into a DnaFile.
    ///
    /// (`dna-util -c my-dna.dna.workdir` creates file `my-dna.dna.gz`)
    #[structopt(short = "c", long)]
    compress: Option<std::path::PathBuf>,
}

async fn run() -> DnaUtilResult<()> {
    let opt = Opt::from_args();

    if opt.expand.is_none() && opt.compress.is_none() {
        eprintln!("INPUT ERROR: no command selected.\n");
        Opt::clap().print_long_help().unwrap();
        return Ok(());
    }

    let mut exclusive: u8 = 0;

    if opt.expand.is_some() {
        exclusive += 1;
    }

    if opt.compress.is_some() {
        exclusive += 1;
    }

    if exclusive > 1 {
        eprintln!("INPUT ERROR: 'extract' and 'compile' commands are exclusive.\n");
        Opt::clap().print_long_help().unwrap();
        return Ok(());
    }

    if let Some(expand) = opt.expand {
        dna_util::expand(&expand).await
    } else if let Some(compress) = opt.compress {
        dna_util::compress(&compress).await
    } else {
        Ok(())
    }
}

/// Main `dna-util` executable entrypoint.
#[tokio::main(threaded_scheduler)]
pub async fn main() {
    if let Err(err) = run().await {
        eprintln!("dna-util: {}", err);
        std::process::exit(1);
    }
}
