use sx_types::observability::{self, Output};
use structopt::StructOpt;
use tokio::runtime::Runtime;
use tracing::*;
use tokio::prelude::*;
use tokio::net;
use tokio::sync;

const RUN_LEN: usize = 1;

#[derive(Debug, StructOpt)]
#[structopt(name = "holochain", about = "The holochain conductor.")]
struct Opt {
    #[structopt(
        long,
        help = "Outputs structured json from logging:
    - None: No logging at all (fastest)
    - Log: Output logs to stdout with spans (human readable)
    - Compact: Same as Log but with less information
    - Json: Output logs as structured json (machine readable)
    ",
        default_value = "Log"
    )]
    structured: Output,
    #[structopt(long)]
    trace_id: Option<String>,
    #[structopt(long)]
    client: Option<String>,
    #[structopt(long)]
    server: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();
    observability::init_fmt(opt.structured.clone(), true).expect("Failed to start contextual logging");
    let mut rt = Runtime::new()?;
    rt.block_on(run(opt));
    Ok(())
}

async fn run(opt: Opt) {
    let (mut tx_s, rx_s) = sync::mpsc::channel(1000);
    let (tx_c, mut rx_c) = sync::mpsc::channel(1000);
    let mut handle = None;
    if let Some(server) = opt.server.clone() {
        handle.replace(tokio::spawn( start_server(server, rx_s)));
    } else if let Some(client) = opt.client.clone() {
        handle.replace(tokio::spawn( start_client(client, tx_c)));
    }
    for _ in 0..RUN_LEN {
        let s = trace_span!("loop");
        let _g = s.enter();
        if let Some(trace_id) = opt.trace_id.clone() {
            let s = trace_span!("trace_root", trace_id = %trace_id);
            let _g = s.enter();
            if opt.server.is_some() {
                let r = tx_s.send(Some(trace_id)).await;
                debug!(?r, "sending");
            }
        }
        if opt.client.is_some() {
            if let Some(trace_id) = rx_c.recv().await {
                let s = trace_span!("follow_span", trace_id = %trace_id);
                let _g = s.enter();
                trace!(%trace_id, "Got id");

            }
        }
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }
    tx_s.try_send(None).ok();
    if let Some(handle) = handle {
        let r = handle.await;
        error!(?r);
    }
}

#[instrument(skip(rx))]
async fn start_server(path: String, mut rx: sync::mpsc::Receiver<Option<String>>) -> Result<(), Box<dyn std::error::Error + Sync + Send>>{
    let mut listner = net::UnixListener::bind(format!("{}/con.sock", path))?;
    let (stream, _) = listner.accept().await?;
    let mut stream = io::BufWriter::new(stream);
    while let Some(Some(msg)) = rx.recv().await {
        stream.write_all(msg.as_bytes()).await?;
    }
    io::AsyncWriteExt::shutdown(&mut stream).await?;
    Ok(())

}

async fn start_client(path: String, mut tx: sync::mpsc::Sender<String>) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let stream = net::UnixStream::connect(format!("{}/con.sock", path)).await?;
    let mut stream = io::BufReader::new(stream);
    let mut input = String::new();
    if let Ok(_) = stream.read_line(&mut input).await {
        tx.send(input.trim().into()).await?;
        input.clear();
    }
    Ok(())
}