use std::path::PathBuf;

use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;
use holochain_p2p::kitsune_p2p::TransportConfig;
use clap::Parser;
use url2::Url2;

// This creates a new Holochain sandbox
// which is a
// - conductor config
// - collection of databases
// - keystore
#[derive(Debug, Parser, Clone)]
pub struct Create {
    /// Number of conductor sandboxes to create.
    #[arg(short, long, default_value = "1")]
    pub num_sandboxes: usize,

    /// Add an optional network config.
    #[command(subcommand)]
    pub network: Option<NetworkCmd>,

    /// Set a root directory for conductor sandboxes to be placed into.
    /// Defaults to the system's temp directory.
    /// This directory must already exist.
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Specify the directory name for each sandbox that is created.
    /// By default, new sandbox directories get a random name
    /// like "kAOXQlilEtJKlTM_W403b".
    /// Use this option to override those names with something explicit.
    /// For example `hc sandbox -r path/to/my/chains -n 3 -d=first,second,third`
    /// will create three sandboxes with directories named "first", "second", and "third".
    #[arg(short, long, value_delimiter = ',')]
    pub directories: Vec<PathBuf>,
}

#[derive(Debug, Parser, Clone)]
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

#[derive(Debug, Parser, Clone)]
pub struct Network {
    /// Set the type of network.
    #[command(subcommand)]
    pub transport: NetworkType,

    /// Optionally set a bootstrap service URL.
    /// A bootstrap service can used for peers to discover each other without
    /// prior knowledge of each other.
    #[arg(short, long, value_parser = try_parse_url2)]
    pub bootstrap: Option<Url2>,
}

#[derive(Debug, Parser, Clone)]
pub enum NetworkType {
    /// A transport that uses the local memory transport protocol.
    Mem,
    /// A transport that uses the QUIC protocol.
    Quic(Quic),
    /// A transport that uses the MDNS protocol.
    Mdns,
    /// A transport that uses the WebRTC protocol.
    #[command(name = "webrtc")]
    WebRTC {
        /// URL to a holochain tx5 WebRTC signal server.
        signal_url: String,
    },
}

#[derive(Debug, Parser, Clone)]
pub struct Quic {
    /// The network interface and port to bind to.
    /// Default: "kitsune-quic://0.0.0.0:0".
    #[arg(short, long, value_parser = try_parse_url2)]
    pub bind_to: Option<Url2>,

    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC IP.
    /// Default: None = use NIC IP.
    #[arg(long)]
    pub override_host: Option<String>,

    /// If you have port-forwarding set up,
    /// you may need to override the local NIC port.
    /// Default: None = use NIC port.
    #[arg(long)]
    pub override_port: Option<u16>,

    /// Run through an external proxy at this URL.
    #[arg(short, value_parser = try_parse_url2)]
    pub proxy: Option<Url2>,
}

#[derive(Debug, Parser, Clone)]
pub struct Existing {
    /// Paths to existing sandbox directories.
    /// For example `hc sandbox run -e=/tmp/kAOXQlilEtJKlTM_W403b,/tmp/kddsajkaasiIII_sJ`.
    #[arg(short, long, value_delimiter = ',')]
    pub existing_paths: Vec<PathBuf>,

    /// Run all the existing conductor sandboxes specified in `$(pwd)/.hc`.
    #[arg(short, long, conflicts_with_all = &["last", "indices"])]
    pub all: bool,

    /// Run the last created conductor sandbox --
    /// that is, the last line in `$(pwd)/.hc`.
    #[arg(short, long, conflicts_with_all = &["all", "indices"])]
    pub last: bool,

    /// Run a selection of existing conductor sandboxes
    /// from those specified in `$(pwd)/.hc`.
    /// Existing sandboxes and their indices are visible via `hc list`.
    /// Use the zero-based index to choose which sandboxes to use.
    /// For example `hc sandbox run 1 3 5` or `hc sandbox run 1`
    #[arg(conflicts_with_all = &["all", "last"])]
    pub indices: Vec<usize>,
}

impl Existing {
    pub fn load(mut self) -> anyhow::Result<Vec<PathBuf>> {
        let sandboxes = crate::save::load(std::env::current_dir()?)?;
        if self.all {
            // Get all the sandboxes
            self.existing_paths.extend(sandboxes.into_iter())
        } else if self.last && sandboxes.last().is_some() {
            // Get just the last sandbox
            self.existing_paths
                .push(sandboxes.last().cloned().expect("Safe due to check above"));
        } else if !self.indices.is_empty() {
            // Get the indices
            let e = self
                .indices
                .into_iter()
                .filter_map(|i| sandboxes.get(i).cloned());
            self.existing_paths.extend(e);
        } else if !self.existing_paths.is_empty() {
            // If there is existing paths then use those
        } else if sandboxes.len() == 1 {
            // If there is only one sandbox then use that
            self.existing_paths
                .push(sandboxes.last().cloned().expect("Safe due to check above"));
        } else if sandboxes.len() > 1 {
            // There is multiple sandboxes, the use must disambiguate
            msg!(
                "
There are multiple sandboxes and hc doesn't know which to run.
You can run:
    - `--all` `-a` run all sandboxes.
    - `--last` `-l` run the last created sandbox.
    - `--existing-paths` `-e` run a list of existing paths to sandboxes.
    - `1` run a sandbox by index from the list below.
    - `0 2` run multiple sandboxes by indices from the list below.
Run `hc sandbox list` to see the sandboxes or `hc sandbox run --help` for more information."
            );
            crate::save::list(std::env::current_dir()?, false)?;
        } else {
            // There are no sandboxes
            msg!(
                "
Before running or calling you need to generate a sandbox.
You can use `hc sandbox generate` to do this.
Run `hc sandbox generate --help` for more options."
            );
        }
        Ok(self.existing_paths)
    }

    pub fn is_empty(&self) -> bool {
        self.existing_paths.is_empty() && self.indices.is_empty() && !self.all && !self.last
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
            NetworkType::Mdns => {
                kit.network_type = holochain_p2p::kitsune_p2p::NetworkType::QuicMdns;
                kit.transport_pool = vec![TransportConfig::Quic {
                    bind_to: None,
                    override_host: None,
                    override_port: None,
                }];
            }
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
                }];
            }
            NetworkType::WebRTC { signal_url } => {
                let transport = TransportConfig::WebRTC { signal_url };
                kit.transport_pool = vec![transport];
            }
        }
        kit
    }
}

impl Default for Create {
    fn default() -> Self {
        Self {
            num_sandboxes: 1,
            network: None,
            root: None,
            directories: Vec::with_capacity(0),
        }
    }
}

// The only purpose for this wrapper function is to get around a type inference failure.
// Plenty of search hits out there for "implementation of `FnOnce` is not general enough"
// e.g., https://users.rust-lang.org/t/implementation-of-fnonce-is-not-general-enough/68294
fn try_parse_url2(arg: &str) -> url2::Url2Result<Url2> {
    Url2::try_parse(arg)
}