use crate::store::Store;

use super::*;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use warp::Filter;

pub(crate) fn put(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply + warp::generic::Tuple, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("X-Op", "put"))
        .and(warp::body::content_length_limit(SIZE_LIMIT))
        .and(warp::body::bytes())
        .and(with_store(store))
        .and_then(put_info)
}

async fn put_info(peer: Bytes, store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    let peer: AgentInfoSigned =
        rmp_decode(&mut AsRef::<[u8]>::as_ref(&peer)).map_err(|_| warp::reject())?;
    // TODO: Return rejection if agent info was invalid?
    if valid(&peer) {
        store.put(peer);
    }
    PUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut buf = Vec::with_capacity(1);
    rmp_encode(&mut buf, ()).map_err(|_| warp::reject())?;
    Ok(buf)
}

fn valid(peer: &AgentInfoSigned) -> bool {
    // TODO: verify signature
    // Verify time
    peer.expires_at_ms as u128
        > std::time::UNIX_EPOCH
            .elapsed()
            .expect("Bootstrap system clock is set before the epoch")
            .as_millis()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use fixt::prelude::*;
    use kitsune_p2p::fixt::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_put() {
        let store = Store::new(vec![]);
        let filter = put(store.clone());

        let info = AgentInfoSigned::sign(
            Arc::new(fixt!(KitsuneSpace, Unpredictable)),
            Arc::new(fixt!(KitsuneAgent, Unpredictable)),
            u32::MAX / 4,
            fixt!(UrlList, Empty),
            0,
            std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        rmp_encode(&mut buf, info.clone()).unwrap();

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "put")
            .body(buf)
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);
        assert_eq!(
            *store
                .all()
                .get(info.space.as_ref())
                .unwrap()
                .get(info.agent.as_ref())
                .unwrap(),
            info
        );
    }
}
