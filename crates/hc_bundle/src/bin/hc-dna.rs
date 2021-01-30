use hc_bundle::cli::HcDnaBundle;
use hc_bundle::error::HcBundleResult;
use structopt::StructOpt;

async fn run() -> HcBundleResult<()> {
    let opt = HcDnaBundle::from_args();

    todo!()

    // if opt.expand.is_none() && opt.compress.is_none() {
    //     eprintln!("INPUT ERROR: no command selected.\n");
    //     Opt::clap().print_long_help().unwrap();
    //     return Ok(());
    // }

    // let mut exclusive: u8 = 0;

    // if opt.expand.is_some() {
    //     exclusive += 1;
    // }

    // if opt.compress.is_some() {
    //     exclusive += 1;
    // }

    // if exclusive > 1 {
    //     eprintln!("INPUT ERROR: 'extract' and 'compile' commands are exclusive.\n");
    //     Opt::clap().print_long_help().unwrap();
    //     return Ok(());
    // }

    // if let Some(expand) = opt.expand {
    //     hc_bundle::dna::expand(&expand).await
    // } else if let Some(compress) = opt.compress {
    //     hc_bundle::dna::compress(&compress).await
    // } else {
    //     Ok(())
    // }
}

/// Main `hc-dna-bundle` executable entrypoint.
#[tokio::main(threaded_scheduler)]
pub async fn main() {
    if let Err(err) = run().await {
        eprintln!("hc-dna-bundle: {}", err);
        std::process::exit(1);
    }
}
