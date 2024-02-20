use anyhow::anyhow;
use clap::Parser;
use holo_hash::{DnaHash, DnaHashB64};
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The websocket URL to connect to the conductor admin API. For example ws://localhost:8000
    #[arg(long)]
    pub admin_url: Option<Url>,

    /// The bootstrap URL to connect to for debugging peer discovery. For example http://localhost:3000
    #[arg(long)]
    pub bootstrap_url: Option<Url>,

    /// The DNA hash in Base64 format to use for
    #[arg(long, value_parser = dna_hash_parser)]
    pub dna_hash: Option<DnaHash>,

    /// The app ID to discover information for
    #[arg(long)]
    pub app_id: Option<String>,
}

impl Args {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(admin_url) = &self.admin_url {
            if admin_url.scheme() != "ws" && admin_url.scheme() != "wss" {
                return Err(anyhow!("Admin URL should use the ws or wss scheme"));
            }
        }

        if let Some(bootstrap_url) = &self.bootstrap_url {
            if bootstrap_url.scheme() != "http" && bootstrap_url.scheme() != "https" {
                return Err(anyhow!("Bootstrap URL should use the http or https scheme"));
            }
        }

        Ok(())
    }
}

fn dna_hash_parser(v: &str) -> anyhow::Result<DnaHash> {
    let raw = DnaHashB64::from_b64_str(v)?;
    Ok(raw.into())
}
