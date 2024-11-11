use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    /// bind to this interface
    #[clap(short, long, default_value = "0.0.0.0:0")]
    interface: String,

    /// include this proxy server address in
    /// `proxy_list` call, can be specified
    /// multiple times
    #[clap(short, long, verbatim_doc_comment)]
    proxy: Vec<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    use std::net::ToSocketAddrs;
    let addr = args
        .interface
        .as_str()
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    match kitsune_p2p_bootstrap::run(addr, args.proxy).await {
        Ok((driver, addr, _shutdown)) => {
            println!("http://{}", addr);
            driver.await;
        }
        Err(err) => eprintln!("{}", err),
    }
}
