use futures::{future::FutureExt, sink::SinkExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::*,
};

/// QUIC implementation of kitsune TransportConnection actor.
struct TransportConnectionQuic {
    quinn_connection: quinn::Connection,
}

impl ghost_actor::GhostControlHandler for TransportConnectionQuic {}

impl ghost_actor::GhostHandler<TransportConnection> for TransportConnectionQuic {}

fn tx_bi_chan(
    mut bi_send: quinn::SendStream,
    mut bi_recv: quinn::RecvStream,
) -> (TransportChannelWrite, TransportChannelRead) {
    let (write_send, mut write_recv) = futures::channel::mpsc::channel::<Vec<u8>>(10);
    let write_send = write_send.sink_map_err(TransportError::other);
    tokio::task::spawn(async move {
        while let Some(data) = write_recv.next().await {
            bi_send
                .write_all(&data)
                .await
                .map_err(TransportError::other)?;
        }
        bi_send.finish().await.map_err(TransportError::other)?;
        TransportResult::Ok(())
    });
    let (mut read_send, read_recv) = futures::channel::mpsc::channel::<Vec<u8>>(10);
    tokio::task::spawn(async move {
        let mut buf = [0_u8; 4096];
        while let Some(read) = bi_recv
            .read(&mut buf)
            .await
            .map_err(TransportError::other)?
        {
            if read == 0 {
                continue;
            }
            read_send
                .send(buf[0..read].to_vec())
                .await
                .map_err(TransportError::other)?;
        }
        TransportResult::Ok(())
    });
    let write_send: TransportChannelWrite = Box::new(write_send);
    let read_recv: TransportChannelRead = Box::new(read_recv);
    (write_send, read_recv)
}

impl TransportConnectionHandler for TransportConnectionQuic {
    fn handle_remote_url(&mut self) -> TransportConnectionHandlerResult<Url2> {
        let out = url2!(
            "{}://{}",
            crate::SCHEME,
            self.quinn_connection.remote_address()
        );
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_create_channel(
        &mut self,
    ) -> TransportConnectionHandlerResult<(TransportChannelWrite, TransportChannelRead)> {
        let maybe_bi = self.quinn_connection.open_bi();
        Ok(async move {
            let (bi_send, bi_recv) = maybe_bi.await.map_err(TransportError::other)?;
            let (write, read) = tx_bi_chan(bi_send, bi_recv);
            Ok((write, read))
        }
        .boxed()
        .into())
    }
}

/// Spawn a new QUIC TransportConnectionSender.
pub(crate) async fn spawn_transport_connection_quic(
    maybe_con: quinn::Connecting,
) -> TransportConnectionResult<(
    ghost_actor::GhostSender<TransportConnection>,
    TransportConnectionEventReceiver,
)> {
    let con = maybe_con.await.map_err(TransportError::other)?;

    let quinn::NewConnection {
        connection,
        mut bi_streams,
        ..
    } = con;

    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TransportConnection>()
        .await?;

    let sender_clone = sender.clone();
    tokio::task::spawn(async move {
        while let Some(Ok((bi_send, bi_recv))) = bi_streams.next().await {
            let sender_clone = sender_clone.clone();
            let incoming_sender = incoming_sender.clone();
            tokio::task::spawn(async move {
                let url = sender_clone
                    .remote_url()
                    .await
                    .map_err(TransportError::other)?;

                let (write, read) = tx_bi_chan(bi_send, bi_recv);

                let _ = incoming_sender.incoming_channel(url, write, read).await;

                TransportResult::Ok(())
            });
        }
    });

    let actor = TransportConnectionQuic {
        quinn_connection: connection,
    };
    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}
