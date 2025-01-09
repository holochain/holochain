use clap::Parser;
use kitsune_p2p_types::config::{KitsuneP2pConfig, TransportConfig};
use url2::Url2;

#[derive(Debug, Parser, Clone)]
pub enum NetworkCmd {
    Network(Network),
}

impl NetworkCmd {
    pub fn as_inner(this: &Option<Self>) -> Option<&Network> {
        match this {
            Some(NetworkCmd::Network(n)) => Some(n),
            None => None,
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
    /// A transport that uses the WebRTC protocol.
    #[command(name = "webrtc")]
    WebRTC {
        /// URL to a holochain tx5 WebRTC signal server.
        signal_url: String,

        /// Optional path to override webrtc peer connection config file.
        webrtc_config: Option<std::path::PathBuf>,
    },
}

impl Network {
    pub async fn to_kitsune(this: &Option<&Self>) -> Option<KitsuneP2pConfig> {
        let Network {
            transport,
            bootstrap,
        } = match this {
            None => return None,
            Some(n) => (*n).clone(),
        };

        let mut kit = KitsuneP2pConfig::mem();
        kit.bootstrap_service = bootstrap;

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
                let transport = TransportConfig::WebRTC {
                    signal_url,
                    webrtc_config,
                };
                kit.transport_pool = vec![transport];
            }
        }
        Some(kit)
    }
}

// The only purpose for this wrapper function is to get around a type inference failure.
// Plenty of search hits out there for "implementation of `FnOnce` is not general enough"
// e.g., https://users.rust-lang.org/t/implementation-of-fnonce-is-not-general-enough/68294
fn try_parse_url2(arg: &str) -> url2::Url2Result<Url2> {
    Url2::try_parse(arg)
}
