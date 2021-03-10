use crate::tx2::tx2_utils::*;
use crate::*;

struct Inner<T: 'static + Send> {
    bucket: Vec<T>,
    notify: Arc<tokio::sync::Notify>,
}

/// Control efficient access to shared resource pool.
pub struct ResourceBucket<T: 'static + Send>(Arc<Share<Inner<T>>>);

impl<T: 'static + Send> Clone for ResourceBucket<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static + Send> Default for ResourceBucket<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static + Send> ResourceBucket<T> {
    /// Create a new resource bucket.
    pub fn new() -> Self {
        Self(Arc::new(Share::new(Inner {
            bucket: Vec::new(),
            notify: Arc::new(tokio::sync::Notify::new()),
        })))
    }

    /// Add a resource to the bucket.
    /// Could be a new resource, or a previously acquired resource.
    pub fn release(&self, t: T) {
        let _ = self.0.share_mut(move |i, _| {
            i.bucket.push(t);
            i.notify.notify();
            Ok(())
        });
    }

    /// Acquire a resource that is immediately available from the bucket
    /// or generate a new one.
    pub fn acquire_or_else<F>(&self, f: F) -> T
    where
        F: FnOnce() -> T + 'static + Send,
    {
        if let Ok(t) = self.0.share_mut(|i, _| {
            if !i.bucket.is_empty() {
                return Ok(i.bucket.remove(0));
            }
            Err(().into())
        }) {
            return t;
        }
        f()
    }

    /// Acquire a resource from the bucket.
    pub fn acquire(
        &self,
        timeout: Option<KitsuneTimeout>,
    ) -> impl std::future::Future<Output = KitsuneResult<T>> + 'static + Send {
        let inner = self.0.clone();
        async move {
            let notify = match inner.share_mut(|i, _| {
                if !i.bucket.is_empty() {
                    return Ok((Some(i.bucket.remove(0)), None));
                }
                Ok((None, Some(i.notify.clone())))
            }) {
                Err(e) => return Err(e),
                Ok((Some(t), None)) => return Ok(t),
                Ok((None, Some(notify))) => notify,
                _ => unreachable!(),
            };
            loop {
                let n = notify.notified();
                match timeout {
                    Some(timeout) => {
                        timeout
                            .mix(async move {
                                n.await;
                                Ok(())
                            })
                            .await
                    }
                    None => {
                        n.await;
                        Ok(())
                    }
                }?;
                match inner.share_mut(|i, _| {
                    if !i.bucket.is_empty() {
                        return Ok(Some(i.bucket.remove(0)));
                    }
                    Ok(None)
                }) {
                    Err(e) => return Err(e),
                    Ok(Some(t)) => return Ok(t),
                    _ => (),
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
        let t = Some(KitsuneTimeout::from_millis(10));
        let bucket = <ResourceBucket<&'static str>>::new();
        let j1 = tokio::task::spawn(bucket.acquire(t));
        let j2 = tokio::task::spawn(bucket.acquire(t));
        assert!(j1.await.unwrap().is_err());
        assert!(j2.await.unwrap().is_err());
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bucket() {
        let bucket = <ResourceBucket<&'static str>>::new();
        let j1 = tokio::task::spawn(bucket.acquire(None));
        let j2 = tokio::task::spawn(bucket.acquire(None));
        bucket.release("1");
        bucket.release("2");
        let j1 = j1.await.unwrap().unwrap();
        let j2 = j2.await.unwrap().unwrap();
        assert!((j1 == "1" && j2 == "2") || (j2 == "1" && j1 == "2"));
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bucket_acquire_or_else() {
        let bucket = <ResourceBucket<&'static str>>::new();
        let j1 = tokio::task::spawn(bucket.acquire(None));
        let j2 = bucket.acquire_or_else(|| "2");
        bucket.release("1");
        let j1 = j1.await.unwrap().unwrap();
        assert_eq!(j1, "1");
        assert_eq!(j2, "2");
    }
}
