//! defines the write/send half of a websocket pair

use crate::*;

/// The sender half allows making outgoing requests to the websocket
/// This struct is cheaply clone-able.
#[derive(Clone)]
pub struct WebsocketSender {
    sender: RawSender,
}

impl WebsocketSender {
    /// internal constructor
    pub(crate) fn priv_new(sender: RawSender) -> Self {
        Self { sender }
    }

    /// Close the websocket
    #[must_use]
    pub fn close(&mut self, code: u16, reason: String) -> BoxFuture<'static, Result<()>> {
        let mut sender = self.sender.clone();
        async move {
            sender
                .send((SendMessage::Close { code, reason }, None))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            Ok(())
        }
        .boxed()
    }

    /// Emit a signal (message without response) to the remote end of this websocket
    #[must_use]
    pub fn signal<SB1>(&mut self, msg: SB1) -> BoxFuture<'static, Result<()>>
    where
        SB1: 'static + std::convert::TryInto<SerializedBytes> + Send,
        <SB1 as std::convert::TryInto<SerializedBytes>>::Error:
            'static + std::error::Error + Send + Sync,
    {
        let span = tracing::debug_span!("sender_signal");
        let mut sender = self.sender.clone();
        async move {
            let bytes: SerializedBytes = msg
                .try_into()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let msg = Message::Signal { data: bytes };

            sender
                .send((SendMessage::Message(msg, span), None))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            Ok(())
        }
        .boxed()
    }

    /// Make a rpc request of the remote end of this websocket
    #[must_use]
    pub fn request<SB1, SB2>(&mut self, msg: SB1) -> BoxFuture<'static, Result<SB2>>
    where
        SB1: 'static + std::convert::TryInto<SerializedBytes> + Send,
        <SB1 as std::convert::TryInto<SerializedBytes>>::Error:
            'static + std::error::Error + Send + Sync,
        SB2: 'static + std::convert::TryFrom<SerializedBytes> + Send,
        <SB2 as std::convert::TryFrom<SerializedBytes>>::Error:
            'static + std::error::Error + Send + Sync,
    {
        let span = tracing::debug_span!("sender_request");
        let mut sender = self.sender.clone();
        async move {
            let bytes: SerializedBytes = msg
                .try_into()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let msg = Message::Request {
                id: nanoid::nanoid!(),
                data: bytes,
            };

            let (send, recv) = tokio::sync::oneshot::channel();

            sender
                .send((SendMessage::Message(msg, span), Some(send)))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            // --
            let bytes: Vec<u8> = recv.await.map_err(|e| Error::new(ErrorKind::Other, e))??;
            let bytes: SerializedBytes = UnsafeBytes::from(bytes).into();
            Ok(SB2::try_from(bytes).map_err(|e| Error::new(ErrorKind::Other, e))?)
        }
        .boxed()
    }
}
