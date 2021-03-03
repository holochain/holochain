use crate::tx2::util::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt, Shared};
use std::collections::HashMap;
use std::sync::atomic;

static MARKER: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

type PendingFut<V> = Shared<BoxFuture<'static, KitsuneResult<V>>>;

#[derive(Clone)]
enum Entry<V>
where
    V: 'static + Send + Sync + Clone,
{
    Pending(usize, PendingFut<V>),
    Ready(V),
}

///
pub struct AsyncMap<K, V>(Share<HashMap<K, Entry<V>>>)
where
    K: 'static + Send + Eq + std::hash::Hash + Clone,
    V: 'static + Send + Sync + Clone;

impl<K, V> Default for AsyncMap<K, V>
where
    K: 'static + Send + Eq + std::hash::Hash + Clone,
    V: 'static + Send + Sync + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> AsyncMap<K, V>
where
    K: 'static + Send + Eq + std::hash::Hash + Clone,
    V: 'static + Send + Sync + Clone,
{
    ///
    pub fn new() -> Self {
        Self(Share::new(HashMap::new()))
    }

    ///
    pub fn remove(&self, k: K) -> KitsuneResult<()> {
        self.0.share_mut(move |i, _| {
            i.remove(&k);
            Ok(())
        })
    }

    ///
    pub fn insert(&self, k: K, v: V) -> KitsuneResult<()> {
        self.0.share_mut(move |i, _| {
            if i.contains_key(&k) {
                return Err("refusing to overwrite AsyncMap entry".into());
            }
            i.insert(k, Entry::Ready(v));
            Ok(())
        })
    }

    ///
    pub fn get<F, C>(
        &self,
        k: K,
        c: C,
    ) -> impl std::future::Future<Output = KitsuneResult<V>> + 'static + Send
    where
        F: std::future::Future<Output = KitsuneResult<V>> + 'static + Send,
        C: FnOnce() -> F + 'static + Send,
    {
        let inner = self.0.clone();
        async move {
            let k2 = k.clone();

            // get an existing entry, or create a new pending one
            let entry = inner.share_mut(move |i, _| {
                Ok(i.entry(k2)
                    .or_insert_with(move || {
                        let fut = c().boxed().shared();
                        Entry::Pending(MARKER.fetch_add(1, atomic::Ordering::Relaxed), fut)
                    })
                    .clone())
            })?;

            // see if we are already read, or still pending
            match entry {
                Entry::Ready(v) => Ok(v),
                Entry::Pending(marker, fut) => {
                    // await the pending fut
                    match fut.await {
                        // if we got a value, see if we need to store it
                        Ok(v) => {
                            let v2 = v.clone();
                            inner.share_mut(move |i, _| {
                                i.entry(k).and_modify(|e| {
                                    let in_marker = if let Entry::Pending(in_marker, _) = e {
                                        *in_marker
                                    } else {
                                        return;
                                    };
                                    if in_marker == marker {
                                        *e = Entry::Ready(v2);
                                    }
                                });
                                Ok(())
                            })?;
                            Ok(v)
                        }
                        // if we got an error, see if we should clear the pending
                        Err(e) => {
                            inner.share_mut(move |i, _| {
                                let in_marker = match i.get(&k) {
                                    None => return Ok(()),
                                    Some(e) => {
                                        if let Entry::Pending(in_marker, _) = e {
                                            *in_marker
                                        } else {
                                            return Ok(());
                                        }
                                    }
                                };
                                if in_marker == marker {
                                    i.remove(&k);
                                }
                                Ok(())
                            })?;
                            Err(e)
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
    async fn test_async_map() {
        let m: AsyncMap<&'static str, &'static str> = AsyncMap::new();

        let err1 = m.get("err", || async { Err("test1".into()) }).boxed();
        let err2 = m.get("err", || async { Err("test2".into()) }).boxed();

        let good1 = m.get("good", || async { Ok("good") }).boxed();
        let good2 = m.get("good", || async { Ok("good") }).boxed();

        let r = futures::future::join_all(vec![err1, err2]).await;
        for r in r {
            assert!(r.is_err());
        }

        let r = futures::future::join_all(vec![good1, good2]).await;
        for r in r {
            assert_eq!("good", r.unwrap());
        }

        let r = m.get("err", || async { Ok("errs_can_then_be_good") }).await;
        assert_eq!("errs_can_then_be_good", r.unwrap());
    }
}
