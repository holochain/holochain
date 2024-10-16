use crate::store::{Store, StoreEntry};

use super::*;
use warp::Filter;

pub(crate) fn put(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply + Sized, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("X-Op", "put"))
        .and(warp::body::content_length_limit(SIZE_LIMIT))
        .and(warp::body::bytes())
        .and(with_store(store))
        .and_then(put_info)
}

async fn put_info(peer: Bytes, store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    #[derive(Debug)]
    struct BadDecode(#[allow(dead_code)] String);
    impl warp::reject::Reject for BadDecode {}
    let peer = StoreEntry::parse(peer.to_vec()).map_err(|e| BadDecode(format!("{e:?}")))?;
    if !valid(&peer) {
        #[derive(Debug)]
        struct Invalid;
        impl warp::reject::Reject for Invalid {}
        return Err(Invalid.into());
    }
    store.put(peer);
    PUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut buf = Vec::with_capacity(1);
    rmp_encode(&mut buf, ()).map_err(|_| warp::reject())?;
    Ok(buf)
}

fn valid(peer: &StoreEntry) -> bool {
    // TODO: actually verify signature... just checking size for now
    if peer.signature.len() != 64 {
        return false;
    }
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
    use ::fixt::prelude::*;
    use kitsune_p2p_bin_data::fixt::*;
    use kitsune_p2p_types::{dht::arq::ArqSize, fixt::*};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_put() {
        let store = Store::new(vec![]);
        let filter = put(store.clone());

        let info = kitsune_p2p_types::agent_info::AgentInfoSigned::sign(
            Arc::new(fixt!(KitsuneSpace, Unpredictable)),
            Arc::new(fixt!(KitsuneAgent, Unpredictable)),
            ArqSize::from_half_len(u32::MAX / 4),
            fixt!(UrlList, Empty),
            0,
            std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
            true,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        rmp_encode(&mut buf, info.clone()).unwrap();

        let mut enc = Vec::new();
        kitsune_p2p_types::codec::rmp_encode(&mut enc, &info).unwrap();
        let info_as_entry = StoreEntry::parse(enc).unwrap();

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
            info_as_entry,
        );
    }
}
