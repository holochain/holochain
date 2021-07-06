//! utility for lazy init-ing things

use futures::future::{BoxFuture, FutureExt, Shared};

/// utility for lazy init-ing things
/// note how new is not async so we can do it in an actor handler
pub struct AsyncLazy<O: 'static + Clone + Send + Sync> {
    fut: Shared<BoxFuture<'static, O>>,
}

impl<O: 'static + Clone + Send + Sync> AsyncLazy<O> {
    /// sync create a new lazy-init value
    /// works best with `Arc<>` types, but anything
    /// `'static + Clone + Send + Sync` will do.
    pub fn new<F>(f: F) -> Self
    where
        F: 'static + std::future::Future<Output = O> + Send,
    {
        Self {
            fut: f.boxed().shared(),
        }
    }

    /// async get the value of this lazy type
    /// will return once the initialization future completes
    pub fn get(&self) -> impl std::future::Future<Output = O> + 'static {
        self.fut.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn async_lazy() {
        let s = AsyncLazy::new(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Arc::new(42)
        });
        assert_eq!(
            vec![Arc::new(42), Arc::new(42)],
            futures::future::join_all(vec![s.get(), s.get()]).await
        );
        assert_eq!(42, *s.get().await);
        assert_eq!(42, *s.get().await);
    }
}
