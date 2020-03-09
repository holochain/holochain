use futures::{executor::ThreadPool, task::SpawnExt};
use std::sync::Arc;
use sx_conductor_lib::{
    api::ExternalConductorApi,
    interface::{channel::ChannelInterface, Interface},
    Conductor,
};
use tokio::sync::{mpsc, RwLock};

fn main() {
    println!("Running silly ChannelInterface example");
    let executor = ThreadPool::new().unwrap();
    futures::executor::block_on(example(executor));
}

async fn example(executor: ThreadPool) {
    let (tx_network, _rx_network) = mpsc::channel(1);
    let (tx_dummy, rx_dummy) = mpsc::unbounded_channel();
    let conductor = Conductor::new(tx_network);
    let lock = Arc::new(RwLock::new(conductor));
    let handle = ExternalConductorApi::new(lock);
    let interface_fut = executor
        .spawn_with_handle(ChannelInterface::new(rx_dummy).spawn(handle))
        .unwrap();
    let driver_fut = executor
        .spawn_with_handle(async move {
            for _ in 0..50 as u32 {
                dbg!("sending dummy msg");
                tx_dummy.send(true).unwrap();
            }
            tx_dummy.send(false).unwrap();
        })
        .unwrap();
    futures::join!(interface_fut, driver_fut);
}
