#![deny(missing_docs)]
//! Binary `dna_util` command executable.

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dna_util", about = "Holochain DnaFile Utility.")]
struct Opt {
    /// Extract a DnaFile into a Dna Working Directory.
    ///
    /// (`dna_util -e my-dna.dna.gz` creates dir `my-dna.dna_work_dir`)
    #[structopt(short = "e", long)]
    extract: Option<std::path::PathBuf>,

    /// Compile a Dna Working Directory into a DnaFile.
    ///
    /// (`dna_util -c my-dna.dna_work_dir` creates file `my-dna.dna.gz`)
    #[structopt(short = "c", long)]
    compile: Option<std::path::PathBuf>,
}

/// Main `dna_util` executable entrypoint.
#[tokio::main(threaded_scheduler)]
pub async fn main() {
    let opt = Opt::from_args();

    if opt.extract.is_none() && opt.compile.is_none() {
        eprintln!("INPUT ERROR: no command selected.\n");
        Opt::clap().print_long_help().unwrap();
        return;
    }

    let mut exclusive = 0;

    if opt.extract.is_some() {
        exclusive += 1;
    }

    if opt.compile.is_some() {
        exclusive += 1;
    }

    if exclusive > 1 {
        eprintln!("INPUT ERROR: 'extract' and 'compile' commands are exclusive.\n");
        Opt::clap().print_long_help().unwrap();
        return;
    }

    if let Some(extract) = opt.extract {
        dna_util::extract(extract).await.unwrap();
    }

    if let Some(compile) = opt.compile {
        dna_util::compile(compile).await.unwrap();
    }

    println!("yo");
}
