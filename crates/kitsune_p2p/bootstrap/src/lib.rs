use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;

use kitsune_p2p_types::codec::rmp_decode;
use kitsune_p2p_types::codec::rmp_encode;
use store::Store;
use warp::{hyper::body::Bytes, Filter};

static NOW: AtomicUsize = AtomicUsize::new(0);
static RANDOM: AtomicUsize = AtomicUsize::new(0);
static PUT: AtomicUsize = AtomicUsize::new(0);

mod clear;
mod now;
mod put;
mod random;
mod store;

/// No reason to accept a peer data bigger then 1KB.
// TODO: Maybe even that's too high?
const SIZE_LIMIT: u64 = 1024;

pub type BootstrapDriver = futures::future::BoxFuture<'static, ()>;

pub async fn run(
    addr: impl Into<SocketAddr> + 'static,
) -> Result<(BootstrapDriver, SocketAddr), String> {
    let store = Store::new();
    let boot = now::now()
        .or(put::put(store.clone()))
        .or(random::random(store.clone()))
        .or(clear::clear(store));
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
