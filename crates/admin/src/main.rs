use holochain_admin as ha;
use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;
use holochain_p2p::kitsune_p2p::TransportConfig;
use holochain_types::prelude::InstalledAppId;
use std::path::PathBuf;
use structopt::StructOpt;
use url2::Url2;

#[derive(Debug, StructOpt)]
/// Holochain Admin - Helper for setting up holochain and making admin requests.
struct Ops {
    #[structopt(subcommand)]
    op: Op,
    /// Force the admin port to a specific value.
    /// Useful if you are setting this config
    /// up for use elsewhere (see also secondary_admin_port).
    #[structopt(short, long)]
    force_admin_port: Option<u16>,
}

#[derive(Debug, StructOpt)]
enum Op {
    /// Create a fresh holochain setup and exit.
    Create(Create),
    /// Run holochain from existing or new setup.
    Run {
        #[structopt(subcommand)]
        /// Choose to run an existing setup or create an new one.
        run: Run,
        #[structopt(short, long)]
        /// Optionally create an app interface.
        port: Option<u16>,
        #[structopt(short, long)]
        /// Specify a secondary admin port so you can make admin requests while holochain is running.
        /// Set to 0 to let OS choose a free port.
        secondary_admin_port: Option<u16>,
    },
}

#[derive(Debug, StructOpt)]
enum Run {
    /// Create a new setup then run.
    New(Create),
    /// Run from an existing setup.
    Existing {
        #[structopt(short, long)]
        /// The path to the existing setup director.
        /// For example `/tmp/kAOXQlilEtJKlTM_W403b`
        path: PathBuf,
    },
}

#[derive(Debug, StructOpt)]
struct Create {
    #[structopt(required = true, min_values = 1)]
    /// List of dnas to run.
    dnas: Vec<PathBuf>,
    #[structopt(subcommand)]
    /// Add an optional network.
    network: Option<Network>,
    #[structopt(short, long, default_value = "test-app")]
    /// Id for the installed app.
    /// This is just a String to identify the app by.
    app_id: InstalledAppId,
}

#[derive(Debug, StructOpt)]
struct Network {
    #[structopt(subcommand)]
    /// Set the type of network.
    transport: NetworkType,
    #[structopt(short, long, parse(from_str = Url2::parse))]
    /// Optionally set a bootstrap url.
    /// The service used for peers to discover each before they are peers.
    bootstrap: Option<Url2>,
}

#[derive(Debug, StructOpt)]
enum NetworkType {
    /// A transport that uses the local memory transport protocol.
    Mem,
    /// A transport that uses the QUIC protocol.
    Quic(Quic),
}

#[derive(Debug, StructOpt)]
struct Quic {
    #[structopt(short, parse(from_str = Url2::parse))]
    /// To which network interface / port should we bind?
    /// Default: "kitsune-quic://0.0.0.0:0".
    bind_to: Option<Url2>,
    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC ip.
    /// Default: None = use NIC ip.
    override_host: Option<String>,
    #[structopt(short)]
    /// If you have port-forwarding set up,
    /// you may need to override the local NIC port.
    /// Default: None = use NIC port.
    override_port: Option<u16>,
    #[structopt(short, parse(from_str = Url2::parse))]
    /// Run through an external proxy at this url.
    proxy: Option<Url2>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::test_run().expect("Failed to start contextual logging");
    let ops = Ops::from_args();
    let create_and_install = |create, force_admin_port| async move {
        let Create {
            dnas,
            network,
            app_id,
        } = create;
        // Create the temp folder and config
        let path = ha::create(network.map(|n| n.into())).await?;
        if let Some(port) = force_admin_port {
            ha::force_admin_port(path.clone(), port)?;
        }
        // Install the app
        ha::install_app(path.clone(), dnas, app_id).await?;
        Result::<_, anyhow::Error>::Ok(path)
    };
    let run_holochain = |path: PathBuf, port, secondary_admin_port, force_admin_port| async move {
        if let Some(secondary_admin_port) = secondary_admin_port {
            let secondary_admin_port = if secondary_admin_port == 0 {
                None
            } else {
                Some(secondary_admin_port)
            };
            ha::add_secondary_admin_port(path.clone(), secondary_admin_port)?;
        }
        ha::run(path, port, force_admin_port).await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    match ops.op {
        Op::Create(create) => {
            create_and_install(create, ops.force_admin_port).await?;
        }
        Op::Run {
            run: Run::Existing { path },
            port,
            secondary_admin_port,
        } => run_holochain(path, port, secondary_admin_port, ops.force_admin_port).await?,
        Op::Run {
            run: Run::New(create),
            port,
            secondary_admin_port,
        } => {
            let path = create_and_install(create, ops.force_admin_port).await?;
            run_holochain(path, port, secondary_admin_port, ops.force_admin_port).await?;
        }
    }

    Ok(())
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
