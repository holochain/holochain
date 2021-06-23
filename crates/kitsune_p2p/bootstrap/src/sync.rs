use std::sync::Arc;

use super::*;
use parking_lot::Mutex;
use tokio::sync::Barrier;
use warp::Filter;

#[derive(Clone)]
pub struct NodeSync {
    waiting: Arc<Mutex<Option<Arc<Barrier>>>>,
}

impl NodeSync {
    pub(crate) fn new() -> Self {
        Self {
            waiting: Arc::new(Mutex::new(None)),
        }
    }
}

pub(crate) fn sync(
    sync: NodeSync,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::header::exact("content-type", "application/octet"))
        .and(warp::header::exact("X-Op", "sync"))
        .and(warp::body::content_length_limit(SIZE_LIMIT))
        .and(warp::body::bytes())
        .and(with_sync(sync))
        .and_then(waiting)
}

async fn waiting(query: Bytes, sync: NodeSync) -> Result<impl warp::Reply, warp::Rejection> {
    let query: u64 = rmp_decode(&mut AsRef::<[u8]>::as_ref(&query)).map_err(|_| warp::reject())?;
    let wait = {
        let mut b = sync.waiting.lock();
        match b.as_mut() {
            Some(b) => b.clone(),
            None => {
                let wait = Arc::new(Barrier::new(query as usize));
                *b = Some(wait.clone());
                wait
            }
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(30), wait.wait())
        .await
        .ok();
    {
        *sync.waiting.lock() = None;
    }
    let mut buf = Vec::with_capacity(1);
    rmp_encode(&mut buf, ()).map_err(|_| warp::reject())?;
    Ok(buf)
}

fn with_sync(
    sync: NodeSync,
) -> impl Filter<Extract = (NodeSync,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || sync.clone())
}
