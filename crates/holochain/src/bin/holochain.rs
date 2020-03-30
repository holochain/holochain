use holochain_2020::conductor::{
    api::ExternalConductorApi,
    interface::{channel::ChannelInterface, Interface},
    Conductor,
};
use std::sync::Arc;
use structopt::StructOpt;
use sx_types::observability::{self, Output};
use tokio::sync::{mpsc, RwLock};
use tracing::*;

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
}

async fn async_main() {
    println!("Running silly ChannelInterface example");
    let opt = Opt::from_args();
    observability::init_fmt(opt.structured).expect("Failed to start contextual logging");
    example().await;
}

fn main() {
    tokio::runtime::Builder::new()
        // we use both IO and Time tokio utilities
        .enable_all()
        // we want to use multiple threads
        .threaded_scheduler()
        // we want to use thread count matching cpu count
        // (sometimes tokio by default only uses half cpu core threads)
        .core_threads(num_cpus::get())
        // give our threads a descriptive name (they'll be numbered too)
        .thread_name("holochain-tokio-thread")
        // build the runtime
        .build()
        // panic if we cannot (we cannot run without it)
        .expect("can build tokio runtime")
        // the async_main function should only end if our program is done
        .block_on(async_main())
}

async fn example() {
    let (tx_network, _rx_network) = mpsc::channel(1);
    let (tx_dummy, rx_dummy) = mpsc::unbounded_channel();
    let conductor = Conductor::new(tx_network);
    let lock = Arc::new(RwLock::new(conductor));
    let handle = ExternalConductorApi::new(lock);
    let interface_fut = ChannelInterface::new(rx_dummy).spawn(handle);
    let driver_fut = async move {
        for _ in 0..50 as u32 {
            debug!("sending dummy msg");
            tx_dummy.send(true).unwrap();
        }
        tx_dummy.send(false).unwrap();
    };
    tokio::join!(interface_fut, driver_fut);
}
