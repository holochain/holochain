use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::serde_json;
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::transport::*;
use std::sync::Arc;
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
    kitsune_p2p_types::metrics::init_sys_info_poll();

    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TlsFileCert {
    #[serde(with = "serde_bytes")]
    pub cert: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub priv_key: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub digest: Vec<u8>,
}

impl From<TlsConfig> for TlsFileCert {
    fn from(f: TlsConfig) -> Self {
        Self {
            cert: f.cert.to_vec(),
            priv_key: f.cert_priv_key.to_vec(),
            digest: f.cert_digest.to_vec(),
        }
    }
}

impl From<TlsFileCert> for TlsConfig {
    fn from(f: TlsFileCert) -> Self {
        Self {
            cert: f.cert.into(),
            cert_priv_key: f.priv_key.into(),
            cert_digest: f.digest.into(),
        }
    }
}

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "kitsune-p2p-proxy")]
struct Opt {
    /// Generate a new self-signed certificate file/priv key and exit.
    /// Danger - this cert is written unencrypted to disk.
    #[structopt(long)]
    pub danger_gen_unenc_cert: Option<std::path::PathBuf>,

    /// Dump a default config file example to stdout and exit.
    #[structopt(long)]
    pub gen_example_config: bool,

    /// Use this config file for proxy configuration.
    #[structopt(default_value = "./proxy-config.yml")]
    pub config_file: std::path::PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Config {
    /// Use a dangerous unencryted tls cert/priv key for this proxy.
    pub danger_use_unenc_cert: Option<std::path::PathBuf>,

    /// To which network interface / port should we bind?
    /// Default: "kitsune-quic://0.0.0.0:0".
    pub bind_to: Option<String>,

    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC ip.
    /// Default: None = use NIC ip.
    pub override_host: Option<String>,

    /// KitsuneP2p Tuning Parameters
    pub tuning_params: Option<KitsuneP2pTuningParams>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            danger_use_unenc_cert: None,
            bind_to: None,
            override_host: None,
            tuning_params: Some(KitsuneP2pTuningParams::default()),
        }
    }
}

impl From<&Config> for kitsune_p2p_transport_quic::ConfigListenerQuic {
    fn from(o: &Config) -> Self {
        let mut out = Self::default();
        if let Some(b) = &o.bind_to {
            out = out.set_bind_to(Some(kitsune_p2p_types::dependencies::url2::url2!("{}", b)));
        }
        if let Some(h) = &o.override_host {
            out = out.set_override_host(Some(h));
        }
        out
    }
}

const EXAMPLE_CONFIG: &[u8] = br#"---
# Use a dangerous unencryted tls cert/priv key for this proxy.
# [OPTIONAL, Default: ephemeral certificate]
danger_use_unenc_cert: "my.cert.file.path"

# To which network interface / port should we bind?
# [OPTIONAL, Default: "kitsune-quic://0.0.0.0:0"]
bind_to: "kitsune-quic://0.0.0.0:0"

# If you have port-forwarding set up,
# or wish to apply a vanity domain name,
# you may need to override the local NIC ip.
# [OPTIONAL, Default: use NIC ip]
override_host: "my.dns.name"

# If you want to override kitsune tuning:
# [OPTIONAL]
tuning_params:
  tls_in_mem_session_storage: 512
  proxy_to_expire_ms: 300000
  concurrent_recv_buffer: 512
  quic_max_idle_timeout: 30000
  quic_connection_channel_limit: 512
  quic_window_multiplier: 1
  quic_crypto_buffer_multiplier: 1
"#;

async fn inner() -> TransportResult<()> {
    let opt = Opt::from_args();

    if let Some(gen_cert) = &opt.danger_gen_unenc_cert {
        let tls = TlsConfig::new_ephemeral().await?;
        let gen_cert2 = gen_cert.clone();
        tokio::task::spawn_blocking(move || {
            let tls = TlsFileCert::from(tls);
            let mut out = Vec::new();
            kitsune_p2p_types::codec::rmp_encode(&mut out, &tls).map_err(TransportError::other)?;
            std::fs::write(gen_cert2, &out).map_err(TransportError::other)?;
            TransportResult::Ok(())
        })
        .await
        .map_err(TransportError::other)??;
        println!("Generated {:?}.", gen_cert);
        return Ok(());
    }

    if opt.gen_example_config {
        let _config: Config = serde_yaml::from_slice(EXAMPLE_CONFIG).unwrap();
        use std::io::Write;
        std::io::stdout()
            .write(EXAMPLE_CONFIG)
            .map_err(TransportError::other)?;
        println!();
        return Ok(());
    }

    let config = opt.config_file.clone();
    let config = match tokio::task::spawn_blocking(move || {
        let config: Config =
            serde_yaml::from_slice(&std::fs::read(config).map_err(TransportError::other)?)
                .map_err(TransportError::other)?;
        TransportResult::Ok(config)
    })
    .await
    .map_err(TransportError::other)
    {
        Ok(Ok(config)) => config,
        _ => Config::default(),
    };

    println!(
        "Executing proxy with config: {}",
        serde_yaml::to_string(&config).unwrap()
    );

    let tuning_params = Arc::new(match &config.tuning_params {
        Some(tuning_params) => tuning_params.clone(),
        None => KitsuneP2pTuningParams::default(),
    });

    let tls_conf = if let Some(use_cert) = &config.danger_use_unenc_cert {
        let use_cert = use_cert.clone();
        tokio::task::spawn_blocking(move || {
            let tls = std::fs::read(use_cert).map_err(TransportError::other)?;
            let tls: TlsFileCert =
                kitsune_p2p_types::codec::rmp_decode(&mut std::io::Cursor::new(&tls))
                    .map_err(TransportError::other)?;
            TransportResult::Ok(TlsConfig::from(tls))
        })
        .await
        .map_err(TransportError::other)??
    } else {
        TlsConfig::new_ephemeral().await?
    };

    let (listener, events) =
        spawn_transport_listener_quic((&config).into(), tuning_params.clone()).await?;

    let proxy_config = ProxyConfig::local_proxy_server(tls_conf, AcceptProxyCallback::accept_all());

    let (listener, events) =
        spawn_kitsune_proxy_listener(proxy_config, tuning_params.clone(), listener, events).await?;

    let listener_clone = listener.clone();
    metric_task(async move {
        loop {
            tokio::time::delay_for(std::time::Duration::from_secs(60)).await;

            let debug_dump = listener_clone.debug().await.unwrap();

            tracing::info!("{}", serde_json::to_string_pretty(&debug_dump).unwrap());
        }

        // needed for types
        #[allow(unreachable_code)]
        <Result<(), ()>>::Ok(())
    });

    println!("{}", listener.bound_url().await?);

    let concurrent_recv = tuning_params.concurrent_recv_buffer as usize;
    metric_task(async move {
        events
            .for_each_concurrent(concurrent_recv, |evt| async {
                match evt {
                    TransportEvent::IncomingChannel(url, mut write, _read) => {
                        tracing::debug!(
                            "{} is trying to talk directly to us - dump proxy state",
                            url
                        );
                        match listener.debug().await {
                            Ok(dump) => {
                                let dump = serde_json::to_string_pretty(&dump).unwrap();
                                let _ = write.write_and_close(dump.into_bytes()).await;
                            }
                            Err(e) => {
                                let _ =
                                    write.write_and_close(format!("{:?}", e).into_bytes()).await;
                            }
                        }
                    }
                }
            })
            .await;
        <Result<(), ()>>::Ok(())
    });

    // wait for ctrl-c
    futures::future::pending().await
}
