use crate::tx2::tx2_utils::*;
use crate::*;
use futures::future::FutureExt;

#[derive(Clone)]
struct ActiveInner(NotifyAll);

impl ActiveInner {
    pub fn new() -> Self {
        Self(NotifyAll::new())
    }

    pub fn kill(&self) {
        self.0.notify();
    }

    pub fn is_active(&self) -> bool {
        !self.0.did_notify()
    }

    pub fn fut<'a, 'b, R, F>(
        &'a self,
        f: F,
    ) -> impl std::future::Future<Output = KitsuneResult<R>> + 'b + Send
    where
        R: 'static + Send,
        F: std::future::Future<Output = KitsuneResult<R>> + 'b + Send,
    {
        let f = f.boxed();
        let not = self.0.wait();
        async move {
            match futures::future::select(f, not).await {
                futures::future::Either::Left((v, _)) => v,
                futures::future::Either::Right(_) => Err(KitsuneErrorKind::Closed.into()),
            }
        }
    }
}

/// Active tracking helper for related items.
/// This facilitates e.g. an endpoint with sub connections.
/// The endpoint can close, closing all connections.
/// Or, individual connections can close, without closing the endpoint.
#[derive(Clone)]
pub struct Active(Box<[ActiveInner]>);

impl Default for Active {
    fn default() -> Self {
        Self::new()
    }
}

impl Active {
    /// Create a new active tracker set to "active".
    pub fn new() -> Self {
        Self(Box::new([ActiveInner::new()]))
    }

    /// Mix two active trackers to gether.
    /// The result will be inactive if either parent is inactive.
    pub fn mix(&self, oth: &Self) -> Self {
        let mut out = self.0.to_vec();
        out.extend_from_slice(&oth.0);
        Self(out.into_boxed_slice())
    }

    /// Kill this active tracker (all trackers if mixed).
    pub fn kill(&self) {
        for i in self.0.iter() {
            i.kill();
        }
    }

    /// If any of the mixed trackers in this instance are not active,
    /// this fn will return false.
    pub fn is_active(&self) -> bool {
        for i in self.0.iter() {
            if !i.is_active() {
                return false;
            }
        }
        true
    }

    /// Wrap a future such that if any of the sub-trackers
    /// within this active tracker instance become inactive
    /// before the future resolve, resolve with a Err::Closed result.
    pub fn fut<'a, 'b, R, F>(
        &'a self,
        f: F,
    ) -> impl std::future::Future<Output = KitsuneResult<R>> + 'b + Send
    where
        R: 'static + Send,
        F: std::future::Future<Output = KitsuneResult<R>> + 'b + Send,
    {
        let mut f = f.boxed();
        for i in self.0.iter() {
            f = i.fut(f).boxed();
        }
        async move { f.await }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_active() {
        let a1 = Active::new();
        let a2 = Active::new();
        let a3 = Active::new();
        let a4 = Active::new();

        let mix = a1.mix(&a2).mix(&a3).mix(&a4);

        assert!(mix.is_active());

        let f1 = mix.fut(async move {
            tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
            Ok(())
        });
        let t1 = tokio::task::spawn(async move {
            assert!(f1.await.is_ok());
        });

        let f2 = mix.fut(async move {
            tokio::time::delay_for(std::time::Duration::from_millis(200)).await;
            Ok(())
        });
        let t2 = tokio::task::spawn(async move {
            assert!(f2.await.is_err());
        });

        tokio::time::delay_for(std::time::Duration::from_millis(120)).await;
        a3.kill();

        t1.await.unwrap();
        t2.await.unwrap();
    }
}
