use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;

use kitsune_p2p_types::codec::rmp_decode;
use kitsune_p2p_types::codec::rmp_encode;
use store::Store;
use warp::{hyper::body::Bytes, Filter};

use crate::sync::NodeSync;

static NOW: AtomicUsize = AtomicUsize::new(0);
static RANDOM: AtomicUsize = AtomicUsize::new(0);
static PUT: AtomicUsize = AtomicUsize::new(0);

mod clear;
mod now;
mod num;
mod put;
mod random;
mod store;
mod sync;

/// No reason to accept a peer data bigger then 1KB.
// TODO: Maybe even that's too high?
const SIZE_LIMIT: u64 = 1024;

/// how often should we prune the expired entries?
const PRUNE_EXPIRED_FREQ_S: u64 = 5;

pub type BootstrapDriver = futures::future::BoxFuture<'static, ()>;

pub async fn run(
    addr: impl Into<SocketAddr> + 'static,
) -> Result<(BootstrapDriver, SocketAddr), String> {
    let store = Store::new();
    let waiter = NodeSync::new();
    {
        let store = store.clone();
        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(PRUNE_EXPIRED_FREQ_S)).await;
                store.prune();
            }
        });
    }
    let boot = now::now()
        .or(put::put(store.clone()))
        .or(random::random(store.clone()))
        .or(clear::clear(store.clone()))
        .or(num::num(store.clone()))
        .or(sync::sync(waiter.clone()));
    match warp::serve(boot).try_bind_ephemeral(addr) {
        Ok((addr, server)) => {
            let driver = futures::future::FutureExt::boxed(server);
            Ok((driver, addr))
        }
        Err(e) => Err(format!("Failed to bind socket: {:?}", e)),
    }
}

fn with_store(
    store: Store,
) -> impl Filter<Extract = (Store,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || store.clone())
}
