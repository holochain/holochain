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
use std::sync::Arc;
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

    /// Message size bytes
    #[structopt(short = "m", long, default_value = "512")]
    message_size_bytes: usize,

    /// Number of incoming requests to process in parallel
    #[structopt(short = "p", long, default_value = "32")]
    parallel_request_handle_count: usize,
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
        ProxyTransport::Quic => {
            let mut cfg = ConfigListenerQuic::default();
            cfg.bind_to = Some(url2::url2!("kitsune-quic://127.0.0.1:0"));
            spawn_transport_listener_quic(
                cfg,
                Arc::new(kitsune_p2p_types::config::KitsuneP2pTuningParams::default()),
            )
            .await
        }
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
        Arc::new(KitsuneP2pTuningParams::default()),
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
        Arc::new(KitsuneP2pTuningParams::default()),
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

    kitsune_p2p_types::metrics::init_sys_info_poll();

    println!("{:#?}", opt);

    let (listener, mut events) = gen_proxy_con(&opt.transport).await?;

    let listener_clone = listener.clone();
    metric_task(async move {
        loop {
            tokio::time::delay_for(std::time::Duration::from_secs(20)).await;

            let debug_dump = listener_clone.debug().await.unwrap();

            println!(
                "{}",
                kitsune_p2p_types::dependencies::serde_json::to_string_pretty(&debug_dump).unwrap()
            );
        }

        // needed for types
        #[allow(unreachable_code)]
        <Result<(), ()>>::Ok(())
    });

    let proxy_url = listener.bound_url().await?;
    println!("Proxy Url: {}", proxy_url);

    // proxy handler - this won't actually do anything
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

    // metrics ticker to wake task if infrequent data
    metric_task({
        let mut metric_send = metric_send.clone();
        async move {
            loop {
                tokio::time::delay_for(std::time::Duration::from_millis(300)).await;
                metric_send
                    .send(Metric::Tick)
                    .await
                    .map_err(TransportError::other)?;
            }
            TransportResult::Ok(())
        }
    });

    // metrics display task
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

    // everybody will talk to this one client
    // this proves that the client can process many responses in parallel
    let (_con, con_url) = gen_client(opt.clone(), proxy_url.clone()).await?;
    println!("Responder Url: {}", con_url);

    // spin up all the nodes that will be making requests of the responder.
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

    let msg_size = opt.message_size_bytes;
    metric_task(async move {
        let in_data = vec![0xdb; msg_size];
        let in_data = &in_data;
        let out_data = vec![0xbd; msg_size];
        let out_data = &out_data;
        let process_delay_ms = opt.process_delay_ms as u64;
        events
            .for_each_concurrent(
                /* limit */ opt.parallel_request_handle_count,
                move |evt| async move {
                    match evt {
                        TransportEvent::IncomingChannel(_url, mut write, read) => {
                            tokio::time::delay_for(std::time::Duration::from_millis(
                                process_delay_ms,
                            ))
                            .await;
                            let data = read.read_to_end().await;
                            assert_eq!(&data, in_data,);
                            let _ = write.write_and_close(out_data.clone()).await;
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
    metric_send: futures::channel::mpsc::Sender<Metric>,
) -> TransportResult<()> {
    let (con, _my_url) = gen_client(opt.clone(), proxy_url).await?;

    let msg_size = opt.message_size_bytes;
    let req_int = opt.request_interval_ms;
    let proc_delay = opt.process_delay_ms;
    loop {
        tokio::time::delay_for(std::time::Duration::from_millis(req_int as u64)).await;

        let out_data = vec![0xdb; msg_size];
        let in_data = vec![0xbd; msg_size];

        let start = std::time::Instant::now();

        let con = con.clone();
        let con_url = con_url.clone();
        let mut metric_send = metric_send.clone();
        metric_task(async move {
            let (_, mut write, read) = con.create_channel(con_url.clone()).await?;
            write.write_and_close(out_data.clone()).await?;
            let res = read.read_to_end().await;
            assert_eq!(in_data, res);
            metric_send
                .send(Metric::RequestOverhead(
                    start.elapsed().as_millis() as u64 - proc_delay as u64,
                ))
                .await
                .map_err(TransportError::other)?;

            TransportResult::Ok(())
        });
    }
}
