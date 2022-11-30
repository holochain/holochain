use crate::store::Store;

use super::*;
use warp::Filter;

pub(crate) fn proxy_list(
    store: Store,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("X-Op", "proxy_list"))
        .and(with_store(store))
        .and_then(get_proxy_list)
}

async fn get_proxy_list(store: Store) -> Result<impl warp::Reply, warp::Rejection> {
    let proxy_list = store.proxy_list();
    let mut buf = Vec::new();
    rmp_encode(&mut buf, proxy_list).map_err(|_| warp::reject())?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_list() {
        let store = Store::new(vec![
            "https://test1.test".into(),
            "https://test2.test".into(),
        ]);
        let filter = super::proxy_list(store.clone());

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "proxy_list")
            .body(vec![])
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);
        let mut result: Vec<String> = rmp_decode(&mut res.body().as_ref()).unwrap();
        assert_eq!(2, result.len());
        assert_eq!("https://test1.test", result.remove(0));
        assert_eq!("https://test2.test", result.remove(0));
    }
}
