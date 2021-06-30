use crate::store::Store;

use super::*;
use warp::Filter;

pub(crate) fn num(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("content-type", "application/octet"))
        .and(warp::header::exact("X-Op", "num"))
        .and(with_store(store))
        .and_then(num_peers)
}

async fn num_peers(store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    let mut buf = Vec::new();
    let peers = store.num() as u64;
    match rmp_encode(&mut buf, peers) {
        Ok(()) => Ok(buf),
        Err(_) => Err(warp::reject()),
    }
}
