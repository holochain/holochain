use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};
use rustls::Session;
use std::io::{Read, Write};

pub(crate) fn spawn_tls_client(
    expected_proxy_url: ProxyUrl,
    tls_client_config: Arc<rustls::ClientConfig>,
    send: TransportChannelWrite,
    recv: TransportChannelRead,
    write: futures::channel::mpsc::Sender<ProxyWire>,
    read: futures::channel::mpsc::Receiver<ProxyWire>,
) -> tokio::sync::oneshot::Receiver<TransportResult<()>> {
    let (setup_send, setup_recv) = tokio::sync::oneshot::channel();
    tokio::task::spawn(tls_client(
        setup_send,
        expected_proxy_url,
        tls_client_config,
        send,
        recv,
        write,
        read,
    ));
    setup_recv
}

async fn tls_client(
    setup_send: tokio::sync::oneshot::Sender<TransportResult<()>>,
    expected_proxy_url: ProxyUrl,
    tls_client_config: Arc<rustls::ClientConfig>,
    mut send: TransportChannelWrite,
    mut recv: TransportChannelRead,
    mut write: futures::channel::mpsc::Sender<ProxyWire>,
    mut read: futures::channel::mpsc::Receiver<ProxyWire>,
) {
    let mut setup_send = Some(setup_send);
    let res: TransportResult<()> = async {
        let nr = webpki::DNSNameRef::try_from_ascii_str("stub.stub").unwrap();
        let mut cli = rustls::ClientSession::new(&tls_client_config, nr);
        let mut buf = [0_u8; 4096];
        let mut in_pre = std::io::Cursor::new(Vec::new());

        let mut outgoing_data_fut = recv.next();
        let mut incoming_wire_fut = read.next();
        let mut did_post_handshake_work = false;
        loop {
            if !did_post_handshake_work && !cli.is_handshaking() {
                did_post_handshake_work = true;

                let cert_digest = blake2b_32(
                    cli.get_peer_certificates()
                        .unwrap()
                        .get(0)
                        .unwrap()
                        .as_ref(),
                );

                let remote_proxy_url =
                    ProxyUrl::new(expected_proxy_url.as_base().as_str(), cert_digest.into())?;
                if let Some(setup_send) = setup_send.take() {
                    if expected_proxy_url == remote_proxy_url {
                        let _ = setup_send.send(Ok(()));
                    } else {
                        let msg = format!(
                            "expected remote {} != received {}",
                            expected_proxy_url, remote_proxy_url,
                        );
                        let _ = setup_send.send(Err(msg.clone().into()));
                        return Err(msg.into());
                    }
                }
            }

            if cli.wants_write() {
                let mut data = Vec::new();
                cli.write_tls(&mut data).map_err(TransportError::other)?;
                write
                    .send(ProxyWire::chan_send(data.into()))
                    .await
                    .map_err(TransportError::other)?;
            }

            if !cli.wants_read() {
                tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                continue;
            }

            use futures::future::Either;
            match futures::future::select(outgoing_data_fut, incoming_wire_fut).await {
                Either::Left((data, fut)) => {
                    match data {
                        Some(data) => {
                            cli.write_all(&data).map_err(TransportError::other)?;
                        }
                        None => return Err("write side shutdown".into()),
                    }
                    outgoing_data_fut = recv.next();
                    incoming_wire_fut = fut;
                }
                Either::Right((wire, fut)) => {
                    match wire {
                        Some(ProxyWire::ChanSend(ChanSend(data))) => {
                            in_pre.get_mut().extend_from_slice(&data);
                            cli.read_tls(&mut in_pre).map_err(TransportError::other)?;
                            cli.process_new_packets().map_err(TransportError::other)?;
                            while let Ok(size) = cli.read(&mut buf) {
                                if size == 0 {
                                    break;
                                }
                                send.send(buf[..size].to_vec()).await?;
                            }
                        }
                        None => return Ok(()),
                        _ => return Err(format!("invalid wire: {:?}", wire).into()),
                    }
                    outgoing_data_fut = fut;
                    incoming_wire_fut = read.next();
                }
            }
        }
    }
    .await;

    if let Err(e) = res {
        let fail = ProxyWire::failure(format!("{:?}", e));
        if let Some(setup_send) = setup_send.take() {
            let _ = setup_send.send(Err(e));
        }
        let _ = write.send(fail).await.map_err(TransportError::other);
    }
}
