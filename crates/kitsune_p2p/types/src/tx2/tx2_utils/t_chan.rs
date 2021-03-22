use crate::tx2::tx2_utils::*;
use crate::*;
use std::future::Future;

use futures::future::FutureExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// tokio::sync::mpsc::Sender is cheaply clonable,
/// futures::channel::mpsc::Sender can be closed from the sender side.
/// We want both these things.
/// Produce a TChan - a wrapper around a tokio::sync::mpsc::channel.
/// Provides futures::stream::Stream impl on TReceive.
/// Allows channel close from sender side.
pub fn t_chan<T: 'static + Send>(bound: usize) -> (TSender<T>, TReceiver<T>) {
    let (s, r) = channel(bound);
    (TSender(Arc::new(Share::new(s))), TReceiver(r))
}

/// The sender side of a t_chan - this is cheaply clone-able.
pub struct TSender<T: 'static + Send>(Arc<Share<Sender<T>>>);

impl<T: 'static + Send> Clone for TSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static + Send> PartialEq for TSender<T> {
    fn eq(&self, oth: &Self) -> bool {
        Arc::ptr_eq(&self.0, &oth.0)
    }
}

impl<T: 'static + Send> Eq for TSender<T> {}

impl<T: 'static + Send> std::hash::Hash for TSender<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl<T: 'static + Send> TSender<T> {
    /// Send a type instance to this channel sender's receiver.
    pub fn send(&self, t: T) -> impl Future<Output = Result<(), T>> + 'static + Send + Unpin {
        let sender = match self.0.share_mut(|i, _| Ok(i.clone())) {
            Err(_) => return async move { Err(t) }.boxed(),
            Ok(s) => s,
        };
        async move { sender.send(t).await.map_err(|e| e.0) }.boxed()
    }

    /// Close this channel from the sender side.
    /// The receiver can accept all pending sends, and then will close.
    pub fn close_channel(&self) {
        let _ = self.0.share_mut(|_, c| {
            *c = true;
            Ok(())
        });
    }
}

/// The receiver side of a t_chan.
pub struct TReceiver<T: 'static + Send>(Receiver<T>);

impl<T: 'static + Send> TReceiver<T> {
    /// Async receive the next item in the stream.
    pub async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }

    /// Poll function for implementing low-level futures streams.
    pub fn poll_recv(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<T>> {
        self.0.poll_recv(cx)
    }
}

impl<T: 'static + Send> futures::stream::Stream for TReceiver<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_t_chan() {
        let (s, r) = t_chan::<u8>(2);

        let f = futures::future::try_join_all(vec![s.send(2), s.send(1), s.send(3)]);

        let t = tokio::task::spawn(async move {
            let f = futures::future::try_join_all(vec![s.send(5), s.send(4), s.send(6)]);
            s.close_channel();
            f.await
        });

        let r = tokio::task::spawn(async move {
            use futures::stream::StreamExt;
            r.collect::<Vec<_>>().await
        });

        f.await.unwrap();
        t.await.unwrap().unwrap();

        let mut r = r.await.unwrap();
        r.sort();
        assert_eq!(&[1, 2, 3, 4, 5, 6], r.as_slice());
    }
}
