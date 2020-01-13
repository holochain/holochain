extern crate crossbeam_channel;

use skunkworx_core::cell::Cell;
use futures::executor::ThreadPool;
use futures::prelude::*;
use skunkworx_conductor_lib::{
    config::Config,
    {api, conductor::Conductor},
};

fn main() {
    let executor = ThreadPool::new().expect("Couldn't create thread pool for conductor");
    // executor.spawn_obj_ok()
    let (tx_network, rx_network) = crossbeam_channel::unbounded();
    let conductor = Conductor::<Cell>::new(tx_network);
}

// trait Interface {
//     fn spawn(self) -> Sender<()>;
// }

// struct DummyInterface;

// impl Interface for DummyInterface {
//     pub fn spawn(self) -> Sender<()> {
//         let (tx, rx) = crossbeam_channel::bounded(0);
//         loop {
//             rx
//         }
//         tx
//     }
// }

// struct ConductorFuture;
