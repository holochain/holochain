use crate::*;
use futures::future::FutureExt;
use std::sync::atomic;
use std::sync::Arc;

/// Represents a single active bool...
/// See Active below, which can be mixed to contain up to 4 of these.
struct ActiveInner {
    /// the actual active boolean value
    act: Arc<atomic::AtomicBool>,

    // below is a temp workaround until tokio 1

    // NOTE - with tokio 1 we'll be able to use Notify
    //        becasue notify_waiters will exist.
    w_send: tokio::sync::broadcast::Sender<bool>,
    // need to capture this receiver just so the sender doesn't close.
    _w_recv: tokio::sync::broadcast::Receiver<bool>,
}

impl ActiveInner {
    pub fn new() -> Self {
        let (w_send, _w_recv) = tokio::sync::broadcast::channel(1);
        Self {
            act: Arc::new(atomic::AtomicBool::new(true)),
            w_send,
            _w_recv,
        }
    }

    pub fn kill(&self) {
        self.act.store(false, atomic::Ordering::SeqCst);
        let _ = self.w_send.send(false);
    }

    pub fn is_active(&self) -> bool {
        self.act.load(atomic::Ordering::SeqCst)
    }
}

/// Active tracking helper for related items.
/// This facilitates e.g. an endpoint with sub connections.
/// The endpoint can close, closing all connections.
/// Or, individual connections can close, without closing the endpoint.
#[derive(Clone)]
pub struct Active([Option<Arc<ActiveInner>>; 4]);

impl Default for Active {
    fn default() -> Self {
        Self::new()
    }
}

impl Active {
    /// Create a new active tracker set to "active".
    pub fn new() -> Self {
        Self([Some(Arc::new(ActiveInner::new())), None, None, None])
    }

    /// Mix two active trackers to gether.
    /// The result will be inactive if either parent is inactive.
    pub fn mix(&self, oth: &Self) -> Self {
        let mut inner = self.0.clone();
        'top: for o in oth.0.iter() {
            if let Some(o) = o {
                for i in inner.iter_mut() {
                    if i.is_none() {
                        *i = Some(o.clone());
                        continue 'top;
                    }
                }
                panic!("No remaining Active slots");
            }
        }
        Self(inner)
    }

    /// Kill this active tracker (all trackers if mixed).
    pub fn kill(&self) {
        for a in self.0.iter() {
            if let Some(a) = a {
                a.kill();
            }
        }
    }

    /// If any of the mixed trackers in this instance are not active,
    /// this fn will return false.
    pub fn is_active(&self) -> bool {
        for a in self.0.iter() {
            if let Some(a) = a {
                if !a.is_active() {
                    return false;
                }
            }
        }
        true
    }

    /// Mutate a future such that if any of the sub-trackers
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
        let mut act_list = Vec::new();
        let mut recv_list = Vec::new();
        for a in self.0.iter() {
            if let Some(a) = a {
                act_list.push(a.act.clone());
                recv_list.push(a.w_send.subscribe());
            }
        }
        async move {
            let w_fut = futures::future::select_all(recv_list.iter_mut().map(|r| r.recv().boxed()));

            // make sure to check this *after* we've registered the
            // watch receive futures.
            for act in act_list.into_iter() {
                if !act.load(atomic::Ordering::SeqCst) {
                    return Err(KitsuneErrorKind::Closed.into());
                }
            }

            let f = f.boxed();
            use futures::future::Either;
            match futures::future::select(w_fut, f).await {
                Either::Left(_) => Err(KitsuneErrorKind::Closed.into()),
                Either::Right((v, _)) => v,
            }
        }
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
