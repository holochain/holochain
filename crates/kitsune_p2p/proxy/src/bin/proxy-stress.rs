use futures::{sink::SinkExt, stream::StreamExt};
use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::{
    config::KitsuneP2pTuningParams,
    dependencies::{ghost_actor, url2},
    metrics::metric_task,
    transport::*,
    transport_mem::*,
};
use structopt::StructOpt;

/// Proxy transport selector
#[derive(structopt::StructOpt, Debug, Clone, Copy)]
pub enum ProxyTransport {
    /// Use the local in-process memory transport (faster, uses more memory)
    Mem,

    /// Use the real QUIC/UDP transport (slower, more realistic)
    Quic,
}

const E: &str = "please specify 'mem'/'m' or 'quic'/'q'";
impl std::str::FromStr for ProxyTransport {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next() {
            Some('M') | Some('m') => Ok(Self::Mem),
            Some('Q') | Some('q') => Ok(Self::Quic),
            _ => Err(E),
        }
    }
}

/// Option Parsing
#[derive(structopt::StructOpt, Debug, Clone)]
#[structopt(name = "proxy-stress")]
pub struct Opt {
    /// Transport to test with. ('mem'/'m' or 'quic'/'q')
    #[structopt(short = "t", long, default_value = "Mem")]
    transport: ProxyTransport,

    /// How many client nodes should be spawned
    #[structopt(short = "n", long, default_value = "128")]
    node_count: u32,

    /// Interval between requests per node
    #[structopt(short = "i", long, default_value = "200")]
    request_interval_ms: u32,

    /// How long nodes should delay before responding
    #[structopt(short = "d", long, default_value = "200")]
    process_delay_ms: u32,
}

#[tokio::main]
async fn main() {
    observability::init_fmt(observability::Output::Compact)
        .expect("Failed to start contextual logging");

    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

async fn gen_base_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    match t {
        ProxyTransport::Mem => spawn_bind_transport_mem().await,
        ProxyTransport::Quic => spawn_transport_listener_quic(Default::default()).await,
    }
}

async fn gen_proxy_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );
    spawn_kitsune_proxy_listener(
        proxy_config,
        KitsuneP2pTuningParams::default(),
        listener,
        events,
    )
    .await
}

async fn gen_cli_con(
    t: &ProxyTransport,
    proxy_url: url2::Url2,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config =
        ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, proxy_url.into());
    spawn_kitsune_proxy_listener(
        proxy_config,
        KitsuneP2pTuningParams::default(),
        listener,
        events,
    )
    .await
}

#[derive(Debug)]
enum Metric {
    Tick,
    RequestOverhead(u64),
}

#[allow(unreachable_code)]
#[allow(clippy::single_match)]
async fn inner() -> TransportResult<()> {
    let opt = Opt::from_args();

    println!("{:#?}", opt);

    let (listener, mut events) = gen_proxy_con(&opt.transport).await?;

    let proxy_url = listener.bound_url().await?;
    println!("Proxy Url: {}", proxy_url);

    metric_task(async move {
        while let Some(evt) = events.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, mut write, _read) => {
                    tracing::debug!("{} is trying to talk directly to us", url);
                    let _ = write.write_and_close(b"".to_vec()).await;
                }
            }
        }
        <Result<(), ()>>::Ok(())
    });

    let (metric_send, mut metric_recv) =
        futures::channel::mpsc::channel((opt.node_count + 10) as usize);

    metric_task({
        let mut metric_send = metric_send.clone();
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                metric_send
                    .send(Metric::Tick)
                    .await
                    .map_err(TransportError::other)?;
            }
            TransportResult::Ok(())
        }
    });

    metric_task(async move {
        let mut last_disp = std::time::Instant::now();
        let mut rtime = Vec::new();
        while let Some(metric) = metric_recv.next().await {
            match metric {
                Metric::RequestOverhead(time) => rtime.push(time),
                _ => (),
            }
            if last_disp.elapsed().as_millis() > 5000 {
                last_disp = std::time::Instant::now();
                let cnt = rtime.len() as f64;
                let mut avg = rtime.drain(..).fold(0.0_f64, |acc, x| acc + (x as f64));
                avg /= cnt;
                println!("Avg Request Overhead ({} requests): {} ms", cnt, avg);
            }
        }
        <Result<(), ()>>::Ok(())
    });

    let (_con, con_url) = gen_client(opt.clone(), proxy_url.clone()).await?;
    println!("Responder Url: {}", con_url);

    for _ in 0..opt.node_count {
        metric_task(client_loop(
            opt.clone(),
            proxy_url.clone(),
            con_url.clone(),
            metric_send.clone(),
        ));
    }

    // wait for ctrl-c
    futures::future::pending().await
}

async fn gen_client(
    opt: Opt,
    proxy_url: url2::Url2,
) -> TransportResult<(ghost_actor::GhostSender<TransportListener>, url2::Url2)> {
    let (con, events) = gen_cli_con(&opt.transport, proxy_url).await?;

    let con_url = con.bound_url().await?;

    metric_task(async move {
        let process_delay_ms = opt.process_delay_ms as u64;
        events
            .for_each_concurrent(
                /* limit */ (opt.node_count + 10) as usize,
                move |evt| async move {
                    match evt {
                        TransportEvent::IncomingChannel(_url, mut write, _read) => {
                            tokio::time::sleep(std::time::Duration::from_millis(process_delay_ms))
                                .await;
                            let _ = write.write_and_close(b"".to_vec()).await;
                        }
                    }
                },
            )
            .await;
        <Result<(), ()>>::Ok(())
    });

    Ok((con, con_url))
}

async fn client_loop(
    opt: Opt,
    proxy_url: url2::Url2,
    con_url: url2::Url2,
    mut metric_send: futures::channel::mpsc::Sender<Metric>,
) -> TransportResult<()> {
    let (con, _my_url) = gen_client(opt.clone(), proxy_url).await?;

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(
            opt.request_interval_ms as u64,
        ))
        .await;

        let start = std::time::Instant::now();
        let (_, mut write, read) = con.create_channel(con_url.clone()).await?;
        write.write_and_close(b"".to_vec()).await?;
        read.read_to_end().await;
        metric_send
            .send(Metric::RequestOverhead(
                start.elapsed().as_millis() as u64 - opt.process_delay_ms as u64,
            ))
            .await
            .map_err(TransportError::other)?;
    }
}
