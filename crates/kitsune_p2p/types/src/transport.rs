//! A collection of definitions related to remote communication.

use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

observability::metrics!(KitsuneTransportMetrics, Write, Read);

/// Error related to remote communication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// GhostError.
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// Unspecified error.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl TransportError {
    /// promote a custom error type to a TransportError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

impl From<String> for TransportError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        TransportError::other(OtherError(s))
    }
}

impl From<&str> for TransportError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<TransportError> for () {
    fn from(_: TransportError) {}
}

impl From<crate::KitsuneError> for TransportError {
    fn from(k: crate::KitsuneError) -> Self {
        TransportError::other(k)
    }
}

/// Result type for remote communication.
pub type TransportResult<T> = Result<T, TransportError>;

/// Receiver side of the channel
pub type TransportChannelRead =
    Box<dyn futures::stream::Stream<Item = Vec<u8>> + Send + Unpin + 'static>;

/// Extension trait for channel readers
pub trait TransportChannelReadExt {
    /// Read the stream to close into a single byte vec.
    fn read_to_end(self)
        -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, Vec<u8>>;
}

impl<T: futures::stream::Stream<Item = Vec<u8>> + Send + Unpin + 'static> TransportChannelReadExt
    for T
{
    fn read_to_end(
        self,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, Vec<u8>> {
        async move {
            let r = self
                .fold(Vec::new(), |mut acc, x| async move {
                    acc.extend_from_slice(&x);
                    acc
                })
                .await;
            KitsuneTransportMetrics::count_filter(
                KitsuneTransportMetrics::Read,
                r.len(),
                "transport",
            );
            r
        }
        .boxed()
        .into()
    }
}

/// Sender side of the channel
pub type TransportChannelWrite =
    Box<dyn futures::sink::Sink<Vec<u8>, Error = TransportError> + Send + Unpin + 'static>;

/// Extension trait for channel writers
pub trait TransportChannelWriteExt {
    /// Write all data and close channel
    fn write_and_close<'a>(
        &'a mut self,
        data: Vec<u8>,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'a, TransportResult<()>>;
}

impl<T: futures::sink::Sink<Vec<u8>, Error = TransportError> + Send + Unpin + 'static>
    TransportChannelWriteExt for T
{
    fn write_and_close<'a>(
        &'a mut self,
        data: Vec<u8>,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'a, TransportResult<()>> {
        KitsuneTransportMetrics::count_filter(
            KitsuneTransportMetrics::Write,
            data.len(),
            "transport",
        );
        async move {
            self.send(data).await?;
            self.close().await?;
            Ok(())
        }
        .boxed()
        .into()
    }
}

/// Sometimes we may need to set up virtual channels,
/// e.g. for translating data before crossing the api boundary.
pub fn create_transport_channel_pair() -> (
    (TransportChannelWrite, TransportChannelRead),
    (TransportChannelWrite, TransportChannelRead),
) {
    let (send1, recv1) = futures::channel::mpsc::channel(10);
    let send1 = send1.sink_map_err(TransportError::other);
    let (send2, recv2) = futures::channel::mpsc::channel(10);
    let send2 = send2.sink_map_err(TransportError::other);

    let send1 = Box::new(send1);
    let recv1 = Box::new(recv1);
    let send2 = Box::new(send2);
    let recv2 = Box::new(recv2);

    ((send1, recv2), (send2, recv1))
}

/// Enum type for events bubbled out of a Transport instance.
pub enum TransportEvent {
    /// A remote is establishing an incoming channel.
    IncomingChannel(url2::Url2, TransportChannelWrite, TransportChannelRead),
}

/// Send new incoming channel data.
pub type TransportEventSender = futures::channel::mpsc::Sender<TransportEvent>;

/// Receiving a new incoming channel connection.
pub type TransportEventReceiver = futures::channel::mpsc::Receiver<TransportEvent>;

ghost_actor::ghost_chan! {
    /// Represents a transport binding for establishing connections.
    /// This api was designed mainly around supporting the QUIC transport.
    /// It should be applicable to other transports, but with some assumptions:
    /// - Keep alive logic should be handled internally.
    /// - Transport encryption is handled internally.
    /// - See light-weight comments below on `create_channel` api.
    pub chan TransportListener<TransportError> {
        /// Grab a debugging internal state dump.
        fn debug() -> serde_json::Value;

        /// Retrieve the current url (address) this listener is bound to.
        fn bound_url() -> url2::Url2;

        /// Attempt to establish an outgoing channel to a remote.
        /// Channels are expected to be very light-weight.
        /// This API was designed around QUIC bi-streams.
        /// If your low-level channels are not light-weight, consider
        /// implementing pooling/multiplex virtual channels to
        /// make this api light weight.
        fn create_channel(url: url2::Url2) -> (
            url2::Url2,
            TransportChannelWrite,
            TransportChannelRead,
        );
    }
}

/// Extension trait for additional methods on TransportListenerSenders
pub trait TransportListenerSenderExt {
    /// Make a request using a single channel open/close.
    fn request(
        &self,
        url: url2::Url2,
        data: Vec<u8>,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, TransportResult<Vec<u8>>>;
}

impl<T: TransportListenerSender> TransportListenerSenderExt for T {
    fn request(
        &self,
        url: url2::Url2,
        data: Vec<u8>,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, TransportResult<Vec<u8>>>
    {
        let fut = self.create_channel(url);
        async move {
            let (_url, mut write, read) = fut.await?;
            KitsuneTransportMetrics::count_filter(
                KitsuneTransportMetrics::Write,
                data.len(),
                "transport",
            );
            write.write_and_close(data).await?;
            let r = read.read_to_end().await;
            KitsuneTransportMetrics::count_filter(
                KitsuneTransportMetrics::Read,
                r.len(),
                "transport",
            );
            Ok(r)
        }
        .boxed()
        .into()
    }
}
