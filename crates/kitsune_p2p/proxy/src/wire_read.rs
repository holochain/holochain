use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::codec::Codec;

/// Wrap a TransportChannelRead in code that decodes ProxyWire items.
pub(crate) fn wrap_wire_read(
    mut read: TransportChannelRead,
) -> futures::channel::mpsc::Receiver<ProxyWire> {
    let (mut send, recv) = futures::channel::mpsc::channel(10);

    metric_task(async move {
        let mut buf = Vec::new();
        while let Some(data) = read.next().await {
            buf.extend_from_slice(&data);
            tracing::trace!("proxy read pending {} bytes", buf.len());
            while let Ok((read_size, wire)) = ProxyWire::decode_ref(&buf) {
                tracing::trace!("proxy read {:?}", wire);
                buf.drain(..(read_size as usize));
                send.send(wire).await.map_err(TransportError::other)?;
            }
        }
        TransportResult::Ok(())
    });

    recv
}
