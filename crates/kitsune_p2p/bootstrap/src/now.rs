use super::*;
use warp::Filter;

pub(crate) fn now(
) -> impl Filter<Extract = impl warp::Reply + warp::generic::Tuple, Error = warp::Rejection> + Clone
{
    warp::post()
        .and(warp::header::exact("X-Op", "now"))
        .and_then(time)
}
async fn time() -> Result<impl warp::Reply, warp::Rejection> {
    let mut buf = Vec::new();
    let ms = std::time::UNIX_EPOCH
        .elapsed()
        .map(|e| e.as_millis() as u64)
        .unwrap_or(0);
    match rmp_encode(&mut buf, ms) {
        Ok(()) => {
            NOW.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(buf)
        }
        Err(_) => Err(warp::reject()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_now() {
        let filter = now();

        let res = warp::test::request()
            .method("POST")
            .header("Content-type", "application/octet")
            .header("X-Op", "now")
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 200);
        let time: u64 = rmp_decode(&mut res.body().as_ref()).unwrap();
        assert!(time > 0);
    }
}
