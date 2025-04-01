use clap::{Parser, Subcommand};
use std::io::{Error, Result};
use std::sync::Arc;

const ONE_KB: [u8; 1024] = [0xdb; 1024];

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Check the health of a bootstrap server.
    Bootstrap {
        /// The url of the bootstrap server to check.
        #[arg(
            short,
            long,
            default_value = "https://dev-test-bootstrap2.holochain.org"
        )]
        url: String,
    },

    /// Check the health of a signal server.
    Signal {
        /// The url of the signal server to check.
        #[arg(short, long, default_value = "wss://dev-test-bootstrap2.holochain.org")]
        url: String,
    },
}

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let Args { cmd } = Args::parse();

    match match cmd {
        Cmd::Bootstrap { url } => bootstrap(url).await,
        Cmd::Signal { url } => signal(url).await,
    } {
        Ok(()) => println!("Done."),
        Err(err) => eprintln!("{err:?}"),
    }
}

async fn bootstrap(url: impl AsRef<str>) -> Result<()> {
    println!("Boostrap check of: {}", url.as_ref());

    let bootstrap_url = url2::Url2::parse(url);

    let bootstrap_url = url2::Url2::parse(format!(
        "{}://{}{}/health",
        bootstrap_url.scheme(),
        bootstrap_url.host().expect("Missing host"),
        bootstrap_url
            .port()
            .map(|p| format!(":{}", p))
            .unwrap_or_default()
    ));
    println!("Checking 'health' at: {bootstrap_url}");

    let health = ureq::get(bootstrap_url.as_str())
        .call()
        .map_err(Error::other)?
        .body_mut()
        .read_to_string()
        .map_err(Error::other)?;

    if health == "{}" {
        println!("Bootstrap server appears healthy");
    } else {
        eprintln!("Got health result: {}", health);
    }

    Ok(())
}

async fn signal(url: String) -> Result<()> {
    println!("Signal check of {url}");
    let config = tx5_signal::SignalConfig {
        listener: false,
        allow_plain_text: true,
        ..Default::default()
    };
    let (conn, _rcv) = tx5_signal::SignalConnection::connect(&url, Arc::new(config)).await?;
    let peer_url = format!("{url}/{:?}", conn.pub_key());
    println!("Got signal connect result: {peer_url}");
    Ok(())
}
