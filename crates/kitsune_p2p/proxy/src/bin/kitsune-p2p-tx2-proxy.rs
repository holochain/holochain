use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::dependencies::{ghost_actor::dependencies::tracing, serde_json};
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_frontend::*;
use kitsune_p2p_types::tx2::tx2_promote::*;
use kitsune_p2p_types::*;
use structopt::StructOpt;

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "kitsune-p2p-tx2-proxy")]
pub struct Opt {
    /// Generate a new self-signed certificate file/priv key and exit.
    /// Danger - this cert is written unencrypted to disk.
    #[structopt(long)]
    pub danger_gen_unenc_cert: Option<std::path::PathBuf>,

    /// Use a dangerous unencryted tls cert/priv key for this proxy.
    #[structopt(long)]
    pub danger_use_unenc_cert: Option<std::path::PathBuf>,

    /// To which network interface / port should we bind?
    #[structopt(short = "b", long, default_value = "kitsune-quic://0.0.0.0:0")]
    pub bind_to: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    observability::test_run().ok();
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

async fn inner() -> KitsuneResult<()> {
    let opt = Opt::from_args();

    if let Some(gen_cert) = &opt.danger_gen_unenc_cert {
        let tls = TlsConfig::new_ephemeral().await?;
        let gen_cert2 = gen_cert.clone();
        tokio::task::spawn_blocking(move || {
            let tls = TlsFileCert::from(tls);
            let mut out = Vec::new();
            kitsune_p2p_types::codec::rmp_encode(&mut out, &tls).map_err(KitsuneError::other)?;
            std::fs::write(gen_cert2, &out).map_err(KitsuneError::other)?;
            KitsuneResult::Ok(())
        })
        .await
        .map_err(KitsuneError::other)??;
        println!("Generated {:?}.", gen_cert);
        return Ok(());
    }

    let tls_conf = if let Some(use_cert) = &opt.danger_use_unenc_cert {
        let use_cert = use_cert.clone();
        tokio::task::spawn_blocking(move || {
            let tls = std::fs::read(use_cert).map_err(KitsuneError::other)?;
            let tls: TlsFileCert =
                kitsune_p2p_types::codec::rmp_decode(&mut std::io::Cursor::new(&tls))
                    .map_err(KitsuneError::other)?;
            KitsuneResult::Ok(TlsConfig::from(tls))
        })
        .await
        .map_err(KitsuneError::other)??
    } else {
        TlsConfig::new_ephemeral().await?
    };

    let mut conf = QuicConfig::default();
    conf.tls = Some(tls_conf.clone());

    let f = QuicBackendAdapt::new(conf).await?;
    let f = tx2_promote(f, 4096 /* max connection count */);
    let f = tx2_proxy(f, tls_conf);

    let ep = f
        .bind(opt.bind_to, KitsuneTimeout::from_millis(30 * 1000))
        .await?;
    println!("{}", ep.handle().local_addr()?);

    let ep_hnd = ep.handle().clone();
    let ep_hnd = &ep_hnd;
    ep.for_each_concurrent(3, move |evt| async move {
        if let EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) = evt {
            let debug = serde_json::json!({
                "proxy": ep_hnd.debug(),
                "sys_info": get_sys_info(),
            });
            let debug = serde_json::to_string_pretty(&debug).unwrap();
            data.clear();
            data.extend_from_slice(debug.as_bytes());
            let t = KitsuneTimeout::from_millis(30 * 3000);
            if let Err(e) = con.write(0.into(), data, t).await {
                tracing::error!("write proxy debug error: {:?}", e);
            }
        }
    })
    .await;

    Ok(())
}
