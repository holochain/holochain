use std::path::PathBuf;

use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;
use holochain_p2p::kitsune_p2p::TransportConfig;
use holochain_types::prelude::InstalledAppId;
use structopt::StructOpt;
use url2::Url2;

const DEFAULT_APP_ID: &str = "test-app";
#[derive(Debug, StructOpt, Clone)]
// This creates a new holochain setup
// which is a
// - conductor config
// - databases
// - keystore
pub struct Create {
    #[structopt(subcommand)]
    /// Add an optional network.
    pub network: Option<NetworkCmd>,
    #[structopt(short, long, default_value = DEFAULT_APP_ID)]
    /// Id for the installed app.
    /// This is just a String to identify the app by.
    pub app_id: InstalledAppId,
    /// Set a root directory for conductor setups to be placed into.
    /// Defaults to your systems temp directory.
    /// This must already exist.
    #[structopt(short, long)]
    pub root: Option<PathBuf>,
    #[structopt(short, long)]
    /// Set a list of subdirectories for each setup that is created.
    /// Defaults to using a random nanoid like: `kAOXQlilEtJKlTM_W403b`.
    /// Theses will be created in the root directory if they don't exist.
    /// For example: `hc gen -r path/to/my/chains -d=first,second,third`
    pub directories: Vec<PathBuf>,
}

#[derive(Debug, StructOpt, Clone)]
pub enum NetworkCmd {
    Network(Network),
}

impl NetworkCmd {
    pub fn into_inner(self) -> Network {
        match self {
            NetworkCmd::Network(n) => n,
        }
    }
}

#[derive(Debug, StructOpt, Clone)]
pub struct Network {
    #[structopt(subcommand)]
    /// Set the type of network.
    pub transport: NetworkType,
    #[structopt(short, long, parse(from_str = Url2::parse))]
    /// Optionally set a bootstrap url.
    /// The service used for peers to discover each before they are peers.
    pub bootstrap: Option<Url2>,
}

#[derive(Debug, StructOpt, Clone)]
pub enum NetworkType {
    /// A transport that uses the local memory transport protocol.
    Mem,
    /// A transport that uses the QUIC protocol.
    Quic(Quic),
}

#[derive(Debug, StructOpt, Clone)]
pub struct Quic {
    #[structopt(short, long, parse(from_str = Url2::parse))]
    /// To which network interface / port should we bind?
    /// Default: "kitsune-quic://0.0.0.0:0".
    pub bind_to: Option<Url2>,
    #[structopt(short, long)]
    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC ip.
    /// Default: None = use NIC ip.
    pub override_host: Option<String>,
    #[structopt(short, long)]
    /// If you have port-forwarding set up,
    /// you may need to override the local NIC port.
    /// Default: None = use NIC port.
    pub override_port: Option<u16>,
    #[structopt(short, parse(from_str = Url2::parse))]
    /// Run through an external proxy at this url.
    pub proxy: Option<Url2>,
}

#[derive(Debug, StructOpt, Clone)]
pub struct Existing {
    #[structopt(short, long, conflicts_with_all = &["dnas", "gen"], value_delimiter = ",")]
    /// Paths to existing setup directories.
    /// For example `hc run -e=/tmp/kAOXQlilEtJKlTM_W403b,/tmp/kddsajkaasiIII_sJ`.
    pub existing_paths: Vec<PathBuf>,
    /// Conductors that have been setup and are
    /// available in `hc list`.
    /// Use the index to choose which setups to use.
    /// For example `hc run -i=1,3,5`
    #[structopt(short = "i", long, conflicts_with_all = &["dnas", "gen"], value_delimiter = ",")]
    pub existing_indices: Vec<usize>,
}

impl Existing {
    pub fn load(mut self) -> anyhow::Result<Vec<PathBuf>> {
        let setups = crate::save::load(std::env::current_dir()?)?;
        let e = self
            .existing_indices
            .into_iter()
            .filter_map(|i| setups.get(i).cloned());
        self.existing_paths.extend(e);
        Ok(self.existing_paths)
    }

    pub fn is_empty(&self) -> bool {
        self.existing_paths.is_empty() && self.existing_indices.is_empty()
    }
}

impl From<Network> for KitsuneP2pConfig {
    fn from(n: Network) -> Self {
        let Network {
            transport,
            bootstrap,
        } = n;
        let mut kit = KitsuneP2pConfig::default();
        kit.bootstrap_service = bootstrap;

        match transport {
            NetworkType::Mem => (),
            NetworkType::Quic(Quic {
                bind_to,
                override_host,
                override_port,
                proxy: None,
            }) => {
                kit.transport_pool = vec![TransportConfig::Quic {
                    bind_to,
                    override_host,
                    override_port,
                }];
            }
            NetworkType::Quic(Quic {
                bind_to,
                override_host,
                override_port,
                proxy: Some(proxy_url),
            }) => {
                let transport = TransportConfig::Quic {
                    bind_to,
                    override_host,
                    override_port,
                };
                kit.transport_pool = vec![TransportConfig::Proxy {
                    sub_transport: Box::new(transport),
                    proxy_config: holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient {
                        proxy_url,
                    },
                }]
            }
        }
        kit
    }
}

impl Default for Create {
    fn default() -> Self {
        Self {
            network: None,
            app_id: DEFAULT_APP_ID.to_string(),
            root: None,
            directories: Vec::with_capacity(0),
        }
    }
}
