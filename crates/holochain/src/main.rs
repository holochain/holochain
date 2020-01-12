extern crate crossbeam_channel;

use futures::executor::ThreadPool;
use futures::prelude::*;
use skunkworx_conductor_lib::config::Config;
use skunkworx_conductor_lib::{api, conductor::Conductor};

fn main() {
    let (tx_api, rx_api) = crossbeam_channel::unbounded();
    let (tx_net, rx_net) = crossbeam_channel::unbounded();
    let executor = ThreadPool::new().expect("Couldn't create thread pool for conductor");
    // executor.spawn_obj_ok()

    let conductor = Conductor::new(executor, rx_api, rx_net);
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

