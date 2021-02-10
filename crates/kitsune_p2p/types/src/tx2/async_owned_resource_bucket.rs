use crate::*;

struct Inner<T: 'static + Send> {
    wait_limit: Arc<tokio::sync::Semaphore>,
    waiting: Option<(
        tokio::sync::OwnedSemaphorePermit,
        tokio::sync::oneshot::Sender<T>,
    )>,
    resources: Vec<T>,
    timeout_ms: Option<u64>,
}

/// Control sync efficient sync access to shared resource pool.
#[derive(Clone)]
pub struct AsyncOwnedResourceBucket<T: 'static + Send> {
    inner: Arc<tokio::sync::Mutex<Inner<T>>>,
}

impl<T: 'static + Send> AsyncOwnedResourceBucket<T> {
    /// Create a new resource bucket.
    pub fn new(timeout_ms: Option<u64>) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Inner {
                wait_limit: Arc::new(tokio::sync::Semaphore::new(1)),
                waiting: None,
                resources: Vec::new(),
                timeout_ms,
            })),
        }
    }

    /// Add a resource to the bucket.
    /// Could be a new resource, or a previously acquired resource.
    pub fn release(&self, t: T) -> impl std::future::Future<Output = ()> + 'static + Send {
        let inner = self.inner.clone();
        async move {
            let mut t = t;
            loop {
                let sender = {
                    let mut inner = inner.lock().await;

                    // if no-one is awaiting, add directly to resource vec
                    if inner.waiting.is_none() {
                        inner.resources.push(t);
                        return;
                    }

                    // if someone is waiting, let's send it to them
                    // also release the waiting permit
                    let (_permit, sender) = inner.waiting.take().unwrap();
                    sender
                };

                // attempt to send - if they are no longer waiting
                // try again to store the resource
                match sender.send(t) {
                    Ok(_) => return,
                    Err(t_) => {
                        t = t_;
                    }
                }
            }
        }
    }

    /// Acquire a resource from the bucket.
    pub fn acquire(&self) -> impl std::future::Future<Output = KitsuneResult<T>> + 'static + Send {
        let inner = self.inner.clone();
        async move {
            // check if a resource is available,
            // or get a space in the waiting line.
            let (permit_fut, timeout_ms) = {
                let mut inner = inner.lock().await;
                if !inner.resources.is_empty() {
                    return Ok(inner.resources.remove(0));
                }
                (inner.wait_limit.clone().acquire_owned(), inner.timeout_ms)
            };

            // await the waiting permit (or maybe timeout)
            tokio::pin!(permit_fut);
            let permit = match timeout_ms {
                None => permit_fut.await,
                Some(timeout_ms) => {
                    match futures::future::select(
                        permit_fut,
                        tokio::time::delay_for(std::time::Duration::from_millis(timeout_ms)),
                    )
                    .await
                    {
                        futures::future::Either::Left((permit, _)) => permit,
                        futures::future::Either::Right(_) => {
                            return Err("timeout".into());
                        }
                    }
                }
            };

            let (s, r) = tokio::sync::oneshot::channel();

            // we're at the head of the line - register ourselves
            // to receive the next resource that becomes available
            {
                let mut inner = inner.lock().await;
                if !inner.resources.is_empty() {
                    return Ok(inner.resources.remove(0));
                }
                // ensure no race-condition / logic problem
                assert!(inner.waiting.is_none());
                inner.waiting = Some((permit, s));
            }

            // now await on our waiting receiver (or maybe timeout)
            match timeout_ms {
                None => r.await.map_err(KitsuneError::other),
                Some(timeout_ms) => {
                    match futures::future::select(
                        r,
                        tokio::time::delay_for(std::time::Duration::from_millis(timeout_ms)),
                    )
                    .await
                    {
                        futures::future::Either::Left((v, _)) => v.map_err(KitsuneError::other),
                        futures::future::Either::Right(_) => {
                            return Err("timeout".into());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bucket_timeout() {
        let bucket = <AsyncOwnedResourceBucket<&'static str>>::new(Some(10));
        let j1 = tokio::task::spawn(bucket.acquire());
        let j2 = tokio::task::spawn(bucket.acquire());
        assert!(j1.await.unwrap().is_err());
        assert!(j2.await.unwrap().is_err());
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bucket() {
        let bucket = <AsyncOwnedResourceBucket<&'static str>>::new(None);
        let j1 = tokio::task::spawn(bucket.acquire());
        let j2 = tokio::task::spawn(bucket.acquire());
        bucket.release("1").await;
        bucket.release("2").await;
        assert_eq!("1", j1.await.unwrap().unwrap());
        assert_eq!("2", j2.await.unwrap().unwrap());
    }
}
