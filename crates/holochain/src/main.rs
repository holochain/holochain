use async_trait::async_trait;
use futures::channel;
use futures::executor::ThreadPool;
use futures::prelude::*;
use futures::task::SpawnExt;
use parking_lot::RwLock;
use skunkworx_conductor_lib::api::{self, ConductorApiExternal, ConductorHandle};
use skunkworx_conductor_lib::interface::puppet::PuppetInterface;
use skunkworx_conductor_lib::interface::Interface;
use skunkworx_conductor_lib::{conductor::Conductor, config::Config};
use skunkworx_core::cell::Cell;
use skunkworx_core::cell::CellApi;
use std::sync::Arc;

fn main() {
    println!("Running silly PuppetInterface example");
    let executor = ThreadPool::new().unwrap();
    futures::executor::block_on(example(executor));
}

async fn example(executor: ThreadPool) {
    let (tx_network, _rx_network) = channel::mpsc::channel(1);
    let (mut tx_dummy, rx_dummy) = channel::mpsc::unbounded();
    let conductor = Conductor::<Cell>::new(tx_network);
    let lock = Arc::new(RwLock::new(conductor));
    let handle = ConductorHandle::new(lock);
    let interface_fut = executor
        .spawn_with_handle(PuppetInterface::new(rx_dummy).spawn(handle))
        .unwrap();
    let driver_fut = executor
        .spawn_with_handle(async move {
            for _ in 0..50 as u32 {
                dbg!("sending dummy msg");
                tx_dummy.send(true).await.unwrap();
            }
            tx_dummy.send(false).await.unwrap();
        })
        .unwrap();
    futures::join!(interface_fut, driver_fut);
}
