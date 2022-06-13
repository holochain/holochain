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
mod proxy_list;
mod put;
mod random;
mod store;

/// No reason to accept a peer data bigger then 1KB.
// TODO: Maybe even that's too high?
const SIZE_LIMIT: u64 = 1024;

/// how often should we prune the expired entries?
pub const PRUNE_EXPIRED_FREQ: std::time::Duration = std::time::Duration::from_secs(5);

pub type BootstrapDriver = futures::future::BoxFuture<'static, ()>;

pub type BootstrapShutdown = Box<dyn FnOnce() + 'static + Send>;

/// Run a bootstrap with the default prune frequency [`PRUNE_EXPIRED_FREQ`].
pub async fn run(
    addr: impl Into<SocketAddr> + 'static,
    proxy_list: Vec<String>,
) -> Result<(BootstrapDriver, SocketAddr, BootstrapShutdown), String> {
    run_with_prune_freq(addr, proxy_list, PRUNE_EXPIRED_FREQ).await
}

/// Run a bootstrap server with a set prune frequency.
pub async fn run_with_prune_freq(
    addr: impl Into<SocketAddr> + 'static,
    proxy_list: Vec<String>,
    prune_frequency: std::time::Duration,
) -> Result<(BootstrapDriver, SocketAddr, BootstrapShutdown), String> {
    let store = Store::new(proxy_list);

    {
        let store = store.clone();
        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(prune_frequency).await;
                store.prune();
            }
        });
    }

    let boot = now::now()
        .or(put::put(store.clone()))
        .or(random::random(store.clone()))
        .or(proxy_list::proxy_list(store.clone()))
        .or(clear::clear(store));

    let (s, r) = tokio::sync::oneshot::channel();
    let shutdown = Box::new(move || {
        let _ = s.send(());
    });

    match warp::serve(boot).try_bind_with_graceful_shutdown(addr, async move {
        let _ = r.await;
    }) {
        Ok((addr, server)) => {
            let driver = futures::future::FutureExt::boxed(server);
            Ok((driver, addr, shutdown))
        }
        Err(e) => Err(format!("Failed to bind socket: {:?}", e)),
    }
}

fn with_store(
    store: Store,
) -> impl Filter<Extract = (Store,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || store.clone())
}
