use async_trait::async_trait;
use futures::channel;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::prelude::*;
use parking_lot::RwLock;
use skunkworx_conductor_lib::api::{self, ConductorApiExternal, ConductorHandle};
use skunkworx_conductor_lib::{
    config::Config,
    {conductor::Conductor},
};
use skunkworx_core::cell::Cell;
use skunkworx_core::cell::CellApi;
use std::sync::Arc;

fn main() {
    dbg!("sanity?");
    let executor = ThreadPool::new().unwrap();
    futures::executor::block_on(run(executor));
}

async fn run(executor: ThreadPool) {
    let (tx_network, _rx_network) = channel::mpsc::channel(1);
    let (tx_dummy, rx_dummy) = channel::mpsc::unbounded();
    let conductor = Conductor::<Cell>::new(tx_network);
    let lock = Arc::new(RwLock::new(conductor));
    let handle = ConductorHandle::new(lock);
    let interface = DummyInterface::new(rx_dummy);
    let interface_fut = interface.spawn(handle);
    let driver_fut = drive_dummy(tx_dummy);
    let i = executor.spawn_with_handle(interface_fut).unwrap();
    let d = executor.spawn_with_handle(driver_fut).unwrap();
    // let i = interface_fut;
    // let d = driver_fut;
    futures::join!(i, d);
}

async fn drive_dummy(mut tx: channel::mpsc::UnboundedSender<bool>) {
    for _i in 0..50 {
        dbg!("sending dummy msg");
        tx.send(true).await.unwrap();
    }
    tx.send(false).await.unwrap();
}

#[async_trait]
trait Interface<Cell: CellApi, Api: ConductorApiExternal<Cell>> {
    async fn spawn(self, api: Api)
    where
        Api: 'async_trait,
        Cell: 'async_trait;
}

struct DummyInterface {
    rx: channel::mpsc::UnboundedReceiver<bool>,
}

impl DummyInterface {
    pub fn new(rx: channel::mpsc::UnboundedReceiver<bool>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl<Cell: CellApi, Api: ConductorApiExternal<Cell>> Interface<Cell, Api> for DummyInterface {
    async fn spawn(mut self, mut api: Api)
    where
        Api: 'async_trait,
        Cell: 'async_trait,
    {
        dbg!("spawn start");
        while let Some(true) = self.rx.next().await {
            dbg!("x");
            api.admin(api::AdminMethod::Start("cell-handle".into()));
        }
    }
}

// struct ConductorFuture;
