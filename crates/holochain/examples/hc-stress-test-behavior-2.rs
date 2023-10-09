use holochain::test_utils::hc_stress_test::*;

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Number of DNAs to install on each conductor node.
    #[arg(long)]
    dna_count: u8,

    /// Number of conductor nodes to run for this test.
    #[arg(long)]
    node_count: u8,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    let test = LocalBehavior2::new(args.dna_count, args.node_count);

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        println!("{:#?}", &*test.lock().unwrap());
    }
}
