use crate::store::Store;

use super::*;
use kitsune_p2p_types::bootstrap::RandomQuery;
use warp::Filter;

pub(crate) fn random(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply + Sized, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("X-Op", "random"))
        .and(warp::body::content_length_limit(SIZE_LIMIT))
        .and(warp::body::bytes())
        .and(with_store(store))
        .and_then(random_info)
}

async fn random_info(query: Bytes, store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    let query: RandomQuery =
        rmp_decode(&mut AsRef::<[u8]>::as_ref(&query)).map_err(|_| warp::reject())?;
    #[derive(serde::Serialize)]
    struct Bin(#[serde(with = "serde_bytes")] Vec<u8>);
    let result = store.random(query).into_iter().map(Bin).collect::<Vec<_>>();
    let mut buf = Vec::with_capacity(result.len());
    rmp_encode(&mut buf, result).map_err(|_| warp::reject())?;
    RANDOM.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use ::fixt::prelude::*;
    use kitsune_p2p::{agent_store::AgentInfoSigned, KitsuneSpace};
    use kitsune_p2p_bin_data::fixt::*;
    use kitsune_p2p_types::{bootstrap::RandomLimit, tx2::tx2_utils::TxUrl};

    async fn put(store: Store, peers: Vec<AgentInfoSigned>) {
        let filter = crate::put::put(store);

        for peer in peers {
            let mut buf = Vec::new();
            rmp_encode(&mut buf, peer).unwrap();

            let res = warp::test::request()
                .method("POST")
                .header("Content-type", "application/octet")
                .header("X-Op", "put")
                .body(buf)
                .reply(&filter)
                .await;
            assert_eq!(res.status(), 200);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_random_returns_offline_nodes() {
        let store = Store::new(vec![]);
        let filter = super::random(store.clone());
        let space: Arc<KitsuneSpace> = Arc::new(fixt!(KitsuneSpace));
        let offline_peer = AgentInfoSigned::sign(
            space.clone(),
            Arc::new(fixt!(KitsuneAgent, Unpredictable)),
            u32::MAX / 4,
            vec![], // no url means offline
            0,
            std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
        )
        .await
        .unwrap();
        put(store.clone(), vec![offline_peer.clone()]).await;

        let query = RandomQuery {
            space,
            limit: RandomLimit(10),
        };
        let mut buf = Vec::new();
        rmp_encode(&mut buf, query).unwrap();

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "random")
            .body(buf)
            .reply(&filter)
            .await;

        assert_eq!(res.status(), 200);
        #[derive(Debug, serde::Deserialize)]
        struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);
        let result: Vec<Bytes> = rmp_decode(&mut res.body().as_ref()).unwrap();

        let mut result: Vec<AgentInfoSigned> = result
            .into_iter()
            .map(|bytes| rmp_decode(&mut AsRef::<[u8]>::as_ref(&bytes.0)).unwrap())
            .collect();

        assert_eq!(result.len(), 1);

        let result = result.remove(0);
        assert_eq!(result, offline_peer);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_random() {
        let store = Store::new(vec![]);
        let filter = super::random(store.clone());
        let space: Arc<KitsuneSpace> = Arc::new(fixt!(KitsuneSpace));
        let mut peers = Vec::new();
        for _ in 0..20 {
            let info = AgentInfoSigned::sign(
                space.clone(),
                Arc::new(fixt!(KitsuneAgent, Unpredictable)),
                u32::MAX / 4,
                vec![TxUrl::from_str_panicking("fake:")],
                0,
                std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
                |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
            )
            .await
            .unwrap();
            peers.push(info);
        }
        put(store.clone(), peers.clone()).await;

        let query = RandomQuery {
            space,
            limit: RandomLimit(10),
        };
        let mut buf = Vec::new();
        rmp_encode(&mut buf, query).unwrap();

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "random")
            .body(buf)
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);
        #[derive(Debug, serde::Deserialize)]
        struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);
        let result: Vec<Bytes> = rmp_decode(&mut res.body().as_ref()).unwrap();
        let result: Vec<AgentInfoSigned> = result
            .into_iter()
            .map(|bytes| rmp_decode(&mut AsRef::<[u8]>::as_ref(&bytes.0)).unwrap())
            .collect();
        for peer in &result {
            assert!(peers.iter().any(|p| p == peer));
        }
        assert_eq!(result.len(), 10);

        // Test different space
        // Test expired
    }
}
