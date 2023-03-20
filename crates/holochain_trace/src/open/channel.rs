use crate::MsgWrap;
use derive_more::{From, Into};
use shrinkwraprs::Shrinkwrap;

pub mod mpsc {
    use super::*;

    #[derive(From, Into, Shrinkwrap)]
    #[shrinkwrap(mutable)]
    pub struct Sender<T>(pub tokio::sync::mpsc::Sender<MsgWrap<T>>);
    #[derive(From, Into, Shrinkwrap)]
    #[shrinkwrap(mutable)]
    pub struct Receiver<T>(pub tokio::sync::mpsc::Receiver<MsgWrap<T>>);

    pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);
        (tx.into(), rx.into())
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T> Sender<T> {
        pub async fn send(
            &mut self,
            value: T,
        ) -> Result<(), tokio::sync::mpsc::error::SendError<T>> {
            self.0
                .send(value.into())
                .await
                .map_err(|e| tokio::sync::mpsc::error::SendError(e.0.without_context()))
        }
    }

    impl<T> Receiver<T> {
        pub async fn recv(&mut self) -> Option<T> {
            self.0.recv().await.map(|t| t.inner())
        }
    }
}

pub mod oneshot {
    use super::*;

    #[derive(From, Into, Shrinkwrap)]
    #[shrinkwrap(mutable)]
    pub struct Sender<T>(pub tokio::sync::oneshot::Sender<MsgWrap<T>>);
    #[derive(From, Into, Shrinkwrap)]
    #[shrinkwrap(mutable)]
    pub struct Receiver<T>(pub tokio::sync::oneshot::Receiver<MsgWrap<T>>);

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (tx.into(), rx.into())
    }

    impl<T> Sender<T> {
        pub fn send(self, value: T) -> Result<(), T> {
            self.0.send(value.into()).map_err(|e| e.without_context())
        }
    }

    impl<T> std::future::Future for Receiver<T> {
        type Output = Result<T, tokio::sync::oneshot::error::RecvError>;

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            use std::task::Poll;
            let p = std::pin::Pin::new(&mut self.0);
            match tokio::sync::oneshot::Receiver::poll(p, cx) {
                Poll::Ready(r) => Poll::Ready(r.map(MsgWrap::inner)),
                Poll::Pending => Poll::Pending,
            }
        }
    }
}
