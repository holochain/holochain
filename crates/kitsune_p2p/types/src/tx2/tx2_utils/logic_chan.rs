use crate::tx2::tx2_utils::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::Stream;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

enum LType<E: 'static + Send> {
    Event(E),
    Logic(OwnedSemaphorePermit, BoxFuture<'static, ()>),
}
type LTypeSend<E> = TSender<LType<E>>;
type LTypeRecv<E> = TReceiver<LType<E>>;

struct LogicChanInner<E: 'static + Send> {
    send: LTypeSend<E>,
    logic_limit: Arc<Semaphore>,
}

/// Handle to a logic_chan instance.
/// A clone of a LogicChanHandle is `Eq` to its origin.
/// A clone of a LogicChanHandle will `Hash` the same as its origin.
pub struct LogicChanHandle<E: 'static + Send>(Share<LogicChanInner<E>>);

impl<E: 'static + Send> Clone for LogicChanHandle<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: 'static + Send> PartialEq for LogicChanHandle<E> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.eq(&oth.0)
    }
}

impl<E: 'static + Send> Eq for LogicChanHandle<E> {}

impl<E: 'static + Send> std::hash::Hash for LogicChanHandle<E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<E: 'static + Send> LogicChanHandle<E> {
    /// Cause the logic_chan to emit an event.
    pub fn emit(
        &self,
        e: E,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        let e = LType::Event(e);
        let send = self.0.share_mut(|i, _| Ok(i.send.clone()));
        async move {
            send?
                .send(e)
                .await
                .map_err(|_| KitsuneError::from(KitsuneErrorKind::Closed))?;
            Ok(())
        }
    }

    /// Capture new logic into the logic_chan.
    /// The passed future can capture other async objects such as streams,
    /// that will be polled as a part of the main logic_chan stream,
    /// without introducing any executor tasks.
    /// Be careful calling `capture_logic()` from within previously captured
    /// logic. While there may be reason to do this, it can lead to
    /// deadlock when approaching the capture_bound.
    pub fn capture_logic<L>(
        &self,
        l: L,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send
    where
        L: std::future::Future<Output = ()> + 'static + Send,
    {
        let l = futures::future::FutureExt::boxed(l);
        let r = self
            .0
            .share_mut(|i, _| Ok((i.logic_limit.clone(), i.send.clone())));
        async move {
            let (limit, send) = r?;
            let permit = limit.acquire_owned().await.map_err(KitsuneError::other)?;
            let l = LType::Logic(permit, l);
            send.send(l)
                .await
                .map_err(|_| KitsuneError::from(KitsuneErrorKind::Closed))?;
            Ok(())
        }
    }

    /// Check if this logic_chan was closed.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Close this logic_chan.
    pub fn close(&self) {
        let _ = self.0.share_mut(|i, c| {
            *c = true;
            i.send.close_channel();
            Ok(())
        });
    }
}

/// A logic channel.
/// Capture a handle to the logic_chan.
/// Fill the LogicChan with async logic.
/// Report events to the handle in the async logic.
/// Treat the LogicChan as a stream, collecting the events.
///
/// # Example
///
/// ```
/// # #[tokio::main(flavor = "multi_thread")]
/// # async fn main() {
/// # use kitsune_p2p_types::tx2::tx2_utils::*;
/// # use futures::stream::StreamExt;
/// let chan = <LogicChan<&'static str>>::new(32);
/// let hnd = chan.handle().clone();
/// hnd.clone().capture_logic(async move {
///     hnd.emit("apple").await.unwrap();
///     hnd.emit("banana").await.unwrap();
///     hnd.close();
/// }).await.unwrap();
///
/// let res = chan.collect::<Vec<_>>().await;
/// assert_eq!(
///     &["apple", "banana"][..],
///     res.as_slice(),
/// );
/// # }
/// ```
pub struct LogicChan<E: 'static + Send> {
    recv: LTypeRecv<E>,
    hnd: LogicChanHandle<E>,
    logic: FuturesUnordered<BoxFuture<'static, ()>>,
}

impl<E: 'static + Send> LogicChan<E> {
    /// Create a new LogicChan instance.
    pub fn new(capture_bound: usize) -> Self {
        let (send, recv) = t_chan(capture_bound);
        let logic_limit = Arc::new(Semaphore::new(capture_bound));
        let inner = LogicChanInner { send, logic_limit };
        let hnd = LogicChanHandle(Share::new(inner));
        let logic = FuturesUnordered::new();
        Self { recv, hnd, logic }
    }

    /// A handle to this logic_chan. You can clone this.
    pub fn handle(&self) -> &LogicChanHandle<E> {
        &self.hnd
    }
}

impl<E: 'static + Send> LogicChan<E> {
    /// if there is any pending logic, poll it to pending
    fn poll_logic(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) {
        loop {
            if self.logic.is_empty() {
                return;
            }

            let l = &mut self.logic;
            futures::pin_mut!(l);
            match Stream::poll_next(l, cx) {
                std::task::Poll::Pending => return,
                std::task::Poll::Ready(None) => return,
                _ => continue,
            }
        }
    }
}

impl<E: 'static + Send> futures::stream::Stream for LogicChan<E> {
    type Item = E;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            // always poll logic to start
            // note, we could make this more efficient by differentiating
            // a custom waker that identified whether logic or stream
            // was woken.
            Self::poll_logic(std::pin::Pin::new(&mut *self), cx);

            // always poll in stream to start
            // (see efficiency note above)
            // this accepts:
            //   - new incoming logic (queued to be polled as we continue loop)
            //   - new events to emit (emitted right away)
            let (permit, new_logic) = {
                match self.recv.poll_recv(cx) {
                    std::task::Poll::Ready(Some(t)) => match t {
                        LType::Event(e) => return std::task::Poll::Ready(Some(e)),
                        LType::Logic(permit, logic) => (permit, logic),
                    },
                    std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            };

            // queue the new logic
            // capture the permit such that it will drop
            // when the logic completes.
            self.logic.push(
                async move {
                    let _permit = permit;
                    new_logic.await;
                }
                .boxed(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic;

    #[tokio::test(flavor = "multi_thread")]
    async fn doc_example() {
        use futures::stream::StreamExt;

        let chan = <LogicChan<&'static str>>::new(32);
        let hnd = chan.handle().clone();
        hnd.clone()
            .capture_logic(async move {
                hnd.emit("apple").await.unwrap();
                hnd.emit("banana").await.unwrap();
                hnd.close();
            })
            .await
            .unwrap();

        let res = chan.collect::<Vec<_>>().await;
        assert_eq!(&["apple", "banana"][..], res.as_slice(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_util_logic_chan() {
        let mut logic_chan = <LogicChan<&'static str>>::new(32);
        let h = logic_chan.handle().clone();
        let a = logic_chan.handle().clone();

        let count = Arc::new(atomic::AtomicUsize::new(0));

        let count2 = count.clone();
        let rt = metric_task(async move {
            while let Some(_res) = futures::stream::StreamExt::next(&mut logic_chan).await {
                count2.fetch_add(1, atomic::Ordering::SeqCst);
            }
            KitsuneResult::Ok(())
        });

        let wt = metric_task(async move {
            a.emit("a1").await.unwrap();
            let b = a.clone();
            a.capture_logic(async move {
                b.emit("b1").await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                b.emit("b2").await.unwrap();
            })
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            a.emit("a2").await.unwrap();
            KitsuneResult::Ok(())
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        h.close();

        wt.await.unwrap().unwrap();
        rt.await.unwrap().unwrap();

        assert_eq!(4, count.load(atomic::Ordering::SeqCst));
    }
}
