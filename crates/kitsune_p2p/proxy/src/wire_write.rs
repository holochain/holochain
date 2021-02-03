use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::dependencies::spawn_pressure;

const MAX_CHANNELS: usize = 500;

/// Wrap a TransportChannelWrite in a sender that takes/encodes ProxyWire items.
pub(crate) async fn wrap_wire_write(
    mut write: TransportChannelWrite,
) -> futures::channel::mpsc::Sender<ProxyWire> {
    let (send, mut recv) = futures::channel::mpsc::channel::<ProxyWire>(10);

    metric_task(spawn_pressure::spawn_limit!(MAX_CHANNELS), async move {
        while let Some(wire) = recv.next().await {
            tracing::trace!("proxy write {:?}", wire);
            let wire = wire.encode_vec().map_err(TransportError::other)?;
            write.send(wire).await?;
        }
        write.close().await?;
        TransportResult::Ok(())
    })
    .await;

    send
}
