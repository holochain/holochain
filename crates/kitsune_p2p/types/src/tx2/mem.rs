#![allow(clippy::new_ret_no_self)]

use crate::tx2::tx_backend::*;
use crate::*;
use futures::{future::FutureExt, stream::StreamExt};

struct ConnectionBackendMemory {}

impl ConnectionBackendMemory {
    pub fn new() -> Arc<dyn ConnectionBackend> {
        Arc::new(Self {})
    }
}

impl ConnectionBackend for ConnectionBackendMemory {
    fn new_outbound(&self, _timeout: KitsuneTimeout) -> OutboundChannelFut {
        unimplemented!()
    }
}

struct MemChanStream {}

impl MemChanStream {
    async fn next(&self) -> KitsuneResult<InboundChannelFut> {
        unimplemented!()
    }
}

struct EndpointBackendMemory {}

impl EndpointBackendMemory {
    pub fn new() -> Arc<dyn EndpointBackend> {
        Arc::new(Self {})
    }
}

impl EndpointBackend for EndpointBackendMemory {
    fn connect(&self, _url: String, _timeout: KitsuneTimeout) -> ConnectionBackendPairFut {
        async move {
            let chan_stream: InboundChannelStream =
                futures::stream::try_unfold(MemChanStream {}, |chan_stream| async move {
                    let con = chan_stream.next().await?;
                    Ok(Some((con, chan_stream)))
                })
                .boxed();

            Ok((ConnectionBackendMemory::new(), chan_stream))
        }
        .boxed()
    }
}

struct MemConnectionStream {}

impl MemConnectionStream {
    async fn next(&self) -> KitsuneResult<ConnectionBackendPairFut> {
        unimplemented!()
    }
}

/// An In-Process Pseuedo transport for testing.
pub struct BindingBackendMemory {}

impl BindingBackendMemory {
    /// Construct a new BindingBackendMemory instance.
    pub fn new() -> Arc<dyn BindingBackend> {
        Arc::new(Self {})
    }
}

impl BindingBackend for BindingBackendMemory {
    fn bind(&self, _url: String, _timeout: KitsuneTimeout) -> EndpointBackendPairFut {
        async move {
            let con_stream: ConnectionStream =
                futures::stream::try_unfold(MemConnectionStream {}, |con_stream| async move {
                    let con = con_stream.next().await?;
                    Ok(Some((con, con_stream)))
                })
                .boxed();

            Ok((EndpointBackendMemory::new(), con_stream))
        }
        .boxed()
    }
}
