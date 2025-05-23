use holochain_conductor_api::conductor::NetworkConfig;
use std::path::PathBuf;

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
    /// For example `hc sandbox generate -r path/to/my/chains -n 3 -d=first,second,third`
    /// will create three sandboxes with directories named "first", "second", and "third".
    #[arg(short, long, value_delimiter = ',')]
    pub directories: Vec<PathBuf>,

    /// Launch Holochain with an embedded lair server instead of a standalone process.
    /// Use this option to run the sandboxed conductors when you don't have access to the lair binary.
    #[arg(long)]
    pub in_process_lair: bool,

    /// Set the conductor config CHC (Chain Head Coordinator) URL
    #[cfg(feature = "chc")]
    #[arg(long, value_parser=try_parse_url2)]
    pub chc_url: Option<Url2>,
}

#[derive(Debug, Parser, Clone)]
pub enum NetworkCmd {
    Network(Network),
}

impl NetworkCmd {
    pub fn as_inner(this: &Option<Self>) -> Option<&Network> {
        match this {
            None => None,
            Some(NetworkCmd::Network(n)) => Some(n),
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
    // /// A transport that uses the QUIC protocol.
    // Quic(Quic),
    // /// A transport that uses the MDNS protocol.
    // Mdns,
    /// A transport that uses the WebRTC protocol.
    #[command(name = "webrtc")]
    WebRTC {
        /// URL to a holochain tx5 WebRTC signal server.
        signal_url: String,

        /// Optional path to override webrtc peer connection config file.
        webrtc_config: Option<std::path::PathBuf>,
    },
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
            self.existing_paths.extend(sandboxes)
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

impl Network {
    pub async fn to_kitsune(this: &Option<&Self>) -> Option<NetworkConfig> {
        let Network {
            transport,
            bootstrap,
        } = match this {
            None => {
                return Some(NetworkConfig {
                    advanced: Some(serde_json::json!({
                        // Allow plaintext signal for hc sandbox to have it work with local
                        // signaling servers spawned by kitsune2-bootstrap-srv
                        "tx5Transport": {
                            "signalAllowPlainText": true,
                        }
                    })),
                    ..NetworkConfig::default()
                });
            }
            Some(n) => (*n).clone(),
        };

        let mut kit = NetworkConfig::default();
        if let Some(bootstrap) = bootstrap {
            kit.bootstrap_url = bootstrap;
        }

        match transport {
            NetworkType::Mem => (),
            NetworkType::WebRTC {
                signal_url,
                webrtc_config,
            } => {
                let webrtc_config = match webrtc_config {
                    Some(path) => {
                        let content = tokio::fs::read_to_string(path)
                            .await
                            .expect("failed to read webrtc_config file");
                        let parsed = serde_json::from_str(&content)
                            .expect("failed to parse webrtc_config file content");
                        Some(parsed)
                    }
                    None => None,
                };
                kit.signal_url = url2::url2!("{}", signal_url);
                kit.webrtc_config = webrtc_config;
                kit.advanced = Some(serde_json::json!({
                    // Allow plaintext signal for hc sandbox to have it work with local
                    // signaling servers spawned by kitsune2-bootstrap-srv
                    "tx5Transport": {
                        "signalAllowPlainText": true,
                    }
                }));
            }
        }
        Some(kit)
    }
}

impl Default for Create {
    fn default() -> Self {
        Self {
            num_sandboxes: 1,
            network: None,
            root: None,
            directories: Vec::with_capacity(0),
            in_process_lair: false,
            #[cfg(feature = "chc")]
            chc_url: None,
        }
    }
}

// The only purpose for this wrapper function is to get around a type inference failure.
// Plenty of search hits out there for "implementation of `FnOnce` is not general enough"
// e.g., https://users.rust-lang.org/t/implementation-of-fnonce-is-not-general-enough/68294
fn try_parse_url2(arg: &str) -> url2::Url2Result<Url2> {
    Url2::try_parse(arg)
}
