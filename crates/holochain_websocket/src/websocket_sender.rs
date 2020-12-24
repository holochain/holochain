//! defines the write/send half of a websocket pair

use super::task_socket_sink::ToSocketSinkSender;
use crate::*;
use task_dispatch_incoming::ToDispatchIncoming;
use task_dispatch_incoming::ToDispatchIncomingSender;
use tracing_futures::Instrument;

/// The Sender/Write half of a split websocket. Use this to make
/// outgoing requests to the remote end of this websocket connection.
/// This struct is cheaply clone-able.
#[derive(Clone)]
pub struct WebsocketSender {
    send_sink: ToSocketSinkSender,
    send_dispatch: ToDispatchIncomingSender,
}

impl WebsocketSender {
    /// internal constructor
    pub(crate) fn priv_new(
        send_sink: ToSocketSinkSender,
        send_dispatch: ToDispatchIncomingSender,
    ) -> Self {
        Self {
            send_sink,
            send_dispatch,
        }
    }

    // FIXME use the code enum not the u16
    /// Close the websocket
    #[must_use]
    pub fn close(&mut self, code: u16, reason: String) -> BoxFuture<'static, Result<()>> {
        let mut send_sink = self.send_sink.clone();
        async move {
            let (send, recv) = tokio::sync::oneshot::channel();

            send_sink
                .send((
                    tungstenite::Message::Close(Some(tungstenite::protocol::CloseFrame {
                        code: code.into(),
                        reason: reason.into(),
                    })),
                    send,
                ))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            recv.await.map_err(|e| Error::new(ErrorKind::Other, e))?;

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
        //let span = tracing::debug_span!("sender_signal");
        let mut send_sink = self.send_sink.clone();
        async move {
            let bytes: SerializedBytes = msg
                .try_into()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let msg = WireMessage::Signal { data: bytes };
            let bytes: SerializedBytes = msg.try_into()?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let msg = tungstenite::Message::Binary(bytes);

            let (send, recv) = tokio::sync::oneshot::channel();

            send_sink
                .send((msg, send))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            recv.await.map_err(|e| Error::new(ErrorKind::Other, e))?;

            Ok(())
        }
        .boxed()
    }

    /// Make a rpc request of the remote end of this websocket
    #[must_use]
    pub fn request<SB1, SB2>(&mut self, msg: SB1) -> BoxFuture<'static, Result<SB2>>
    where
        SB1: 'static + std::convert::TryInto<SerializedBytes> + Send + std::fmt::Debug,
        <SB1 as std::convert::TryInto<SerializedBytes>>::Error:
            'static + std::error::Error + Send + Sync,
        SB2: 'static + std::convert::TryFrom<SerializedBytes> + Send,
        <SB2 as std::convert::TryFrom<SerializedBytes>>::Error:
            'static + std::error::Error + Send + Sync,
    {
        let mut send_sink = self.send_sink.clone();
        let mut send_dispatch = self.send_dispatch.clone();
        async move {
            tracing::trace!(request_msg = ?msg);
            let bytes: SerializedBytes = msg
                .try_into()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let id = nanoid::nanoid!();

            let (send_response, recv_response) = tokio::sync::oneshot::channel();

            send_dispatch
                .send(ToDispatchIncoming::RegisterResponse {
                    id: id.clone(),
                    respond: send_response,
                })
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            let msg = WireMessage::Request { id, data: bytes };
            let bytes: SerializedBytes = msg.try_into()?;
            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

            let msg = tungstenite::Message::Binary(bytes);

            let (send_complete, recv_complete) = tokio::sync::oneshot::channel();

            send_sink
                .send((msg, send_complete))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            recv_complete
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            let bytes = recv_response
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))??;
            let bytes: SerializedBytes = UnsafeBytes::from(bytes).into();
            Ok(SB2::try_from(bytes).map_err(|e| Error::new(ErrorKind::Other, e))?)
        }
        .instrument(tracing::debug_span!("sender_request"))
        .boxed()
    }
}
