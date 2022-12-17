use crate::store::Store;

use super::*;
use warp::Filter;

pub(crate) fn clear(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply + warp::generic::Tuple, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("X-Op", "clear"))
        .and(with_store(store))
        .and_then(clear_info)
}

async fn clear_info(store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    store.clear();
    Ok(warp::reply())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use fixt::prelude::*;
    use kitsune_p2p::{agent_store::AgentInfoSigned, fixt::*, KitsuneSpace};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_clear() {
        let store = Store::new(vec![]);

        let filter = super::clear(store.clone());
        let space: Arc<KitsuneSpace> = Arc::new(fixt!(KitsuneSpace));

        for _ in 0..20 {
            let info = AgentInfoSigned::sign(
                space.clone(),
                Arc::new(fixt!(KitsuneAgent, Unpredictable)),
                u32::MAX / 4,
                fixt!(UrlList, Empty),
                0,
                std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
                |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
            )
            .await
            .unwrap();
            store.put(info);
        }

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "clear")
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);
        assert!(store.all().is_empty());
    }
}
