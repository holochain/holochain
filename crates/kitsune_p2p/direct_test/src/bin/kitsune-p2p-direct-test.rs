use structopt::StructOpt;

#[tokio::main]
async fn main() {
    observability::init_fmt(observability::Output::LogTimed).ok();

    let opt = kitsune_p2p_direct_test::Opt::from_args();

    if let Err(e) = kitsune_p2p_direct_test::execute(opt).await {
        eprintln!("{:?}", e);
    }
}
