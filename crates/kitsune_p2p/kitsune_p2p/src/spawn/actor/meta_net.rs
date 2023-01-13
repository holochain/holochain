// because of feature flipping
#![allow(dead_code)]
#![allow(irrefutable_let_patterns)]
#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(clippy::needless_return)]
#![allow(clippy::blocks_in_if_conditions)]
//! Networking abstraction to handle feature flipping.

use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

#[cfg(feature = "tx2")]
use kitsune_p2p_proxy::tx2::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_transport_quic::tx2::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_api::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_restart_adapter::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::*;

use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::*;
use std::sync::Arc;

pub type RespondFut = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static + Send>>;

pub type Respond = Box<dyn FnOnce(wire::Wire) -> RespondFut + 'static + Send>;

/// Events emitted by a meta net instance.
pub enum MetaNetEvt {
    /// A connection has been established.
    Connected {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,
    },

    /// A connection has been closed.
    Disconnected {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,
    },

    /// An incoming request expecting a response.
    Request {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,

        /// The request data sent by the remote peer.
        data: wire::Wire,

        /// Respond to this request.
        respond: Respond,
    },

    /// An incoming notification that doesn't require a direct response.
    Notify {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,

        /// The request data sent by the remote peer.
        data: wire::Wire,
    },
}

pub type MetaNetEvtRecv = futures::channel::mpsc::Receiver<MetaNetEvt>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaNetCon {
    #[cfg(feature = "tx2")]
    Tx2(Tx2ConHnd<wire::Wire>),

    #[cfg(feature = "tx4")]
    Tx4(tx4::Ep, tx4::Tx4Url),
}

impl MetaNetCon {
    pub async fn close(&self, code: u32, reason: &str) {
        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con) = self {
                con.close(code, reason).await;
                return;
            }
        }

        // TODO - no way to close a tx4 con currently
    }

    pub fn is_closed(&self) -> bool {
        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con) = self {
                return con.is_closed();
            }
        }

        #[cfg(feature = "tx4")]
        {
            // TODO - erm, tx4 connections are never exactly "closed"
            //        since it's more of a message queue...
            return false;
        }

        true
    }

    pub async fn notify(&self, payload: &wire::Wire, timeout: KitsuneTimeout) -> KitsuneResult<()> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con) = self {
                return con.notify(payload, timeout).await;
            }
        }

        #[cfg(feature = "tx4")]
        {
            if let MetaNetCon::Tx4(ep, rem_url) = self {
                let mut writer = tx4::Buf::from_writer().map_err(KitsuneError::other)?;
                use kitsune_p2p_types::codec::Codec;
                payload.encode(&mut writer).map_err(KitsuneError::other)?;
                let data = writer.finish();
                ep.send(rem_url.clone(), data)
                    .await
                    .map_err(KitsuneError::other)?;
                return Ok(());
            }
        }

        Err("invalid features".into())
    }

    pub async fn request(
        &self,
        payload: &wire::Wire,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<wire::Wire> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con) = self {
                return con.request(payload, timeout).await;
            }
        }

        Err("invalid features".into())
    }

    pub fn peer_id(&self) -> Arc<[u8; 32]> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con) = self {
                return con.peer_cert().into();
            }
        }

        panic!("invalid features");
    }
}

/// Networking abstraction to handle feature flipping.
#[derive(Debug, Clone)]
pub enum MetaNet {
    /// Tx2 Abstraction
    #[cfg(feature = "tx2")]
    Tx2(Tx2EpHnd<wire::Wire>),

    /// Tx4 Abstraction
    #[cfg(feature = "tx4")]
    Tx4(tx4::Ep, tx4::Tx4Url),
}

impl MetaNet {
    /// Construct abstraction with tx2 backend.
    #[cfg(feature = "tx2")]
    pub async fn new_tx2(
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_types::tls::TlsConfig,
        metrics: Tx2ApiMetrics,
    ) -> KitsuneP2pResult<(Self, MetaNetEvtRecv)> {
        let tuning_params = config.tuning_params.clone();
        let (mut evt_send, evt_recv) =
            futures::channel::mpsc::channel(tuning_params.concurrent_limit_per_thread);

        let tx2_conf = config.to_tx2().map_err(KitsuneP2pError::other)?;

        let mut is_mock = false;

        // set up our backend based on config
        let (f, bind_to) = match tx2_conf.backend {
            KitsuneP2pTx2Backend::Mem => {
                let mut conf = MemConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_mem_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    "none:".into(),
                )
            }
            KitsuneP2pTx2Backend::Quic { bind_to } => {
                let mut conf = QuicConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_quic_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    bind_to,
                )
            }
            KitsuneP2pTx2Backend::Mock { mock_network } => {
                is_mock = true;
                (mock_network, "none:".into())
            }
        };

        // wrap in restart logic
        let f = tx2_restart_adapter(f);

        // convert to frontend
        let f = tx2_pool_promote(f, config.tuning_params.clone());

        // wrap in proxy
        let f = if !is_mock {
            let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
            conf.tuning_params = Some(config.tuning_params.clone());
            match tx2_conf.use_proxy {
                KitsuneP2pTx2ProxyConfig::NoProxy => (),
                KitsuneP2pTx2ProxyConfig::Specific(proxy_url) => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Specific(proxy_url);
                }
                KitsuneP2pTx2ProxyConfig::Bootstrap {
                    bootstrap_url,
                    fallback_proxy_url,
                } => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Bootstrap {
                        bootstrap_url,
                        fallback_proxy_url,
                    };
                    conf.proxy_from_bootstrap_cb = Arc::new(|bootstrap_url| {
                        Box::pin(async move {
                            match crate::spawn::actor::bootstrap::proxy_list(bootstrap_url.into())
                                .await
                            {
                                Ok(mut proxy_list) => {
                                    if proxy_list.is_empty() {
                                        return None;
                                    }
                                    use rand::Rng;
                                    Some(
                                        proxy_list
                                            .remove(
                                                rand::thread_rng().gen_range(0..proxy_list.len()),
                                            )
                                            .into(),
                                    )
                                }
                                _ => None,
                            }
                        })
                    });
                }
            }
            let f = tx2_proxy(f, conf)?;
            f
        } else {
            f
        };

        // wrap in api
        let f = tx2_api(f, metrics);

        // bind local endpoint
        let mut ep = f
            .bind(bind_to, config.tuning_params.implicit_timeout())
            .await
            .map_err(KitsuneP2pError::other)?;

        // capture endpoint handle
        let ep_hnd = ep.handle().clone();

        tokio::task::spawn(async move {
            let tuning_params = &tuning_params;
            while let Some(evt) = ep.next().await {
                match evt {
                    Tx2EpEvent::OutgoingConnection(Tx2EpConnection { con, url }) => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingConnection(Tx2EpConnection { con, url }) => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::ConnectionClosed(Tx2EpConnectionClosed { con, url, .. }) => {
                        if evt_send
                            .send(MetaNetEvt::Disconnected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest {
                        con,
                        url,
                        data,
                        respond,
                    }) => {
                        let timeout = tuning_params.implicit_timeout();
                        if evt_send
                            .send(MetaNetEvt::Request {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con),
                                data,
                                respond: Box::new(move |data| {
                                    let out: RespondFut = Box::pin(async move {
                                        let _ = respond.respond(data, timeout).await;
                                    });
                                    out
                                }),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        if evt_send
                            .send(MetaNetEvt::Notify {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con),
                                data,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::Error(_) | Tx2EpEvent::Tick | Tx2EpEvent::EndpointClosed => (),
                }
            }
        });

        Ok((MetaNet::Tx2(ep_hnd), evt_recv))
    }

    /// Construct abstraction with tx4 backend.
    #[cfg(feature = "tx4")]
    pub async fn new_tx4(
        tuning_params: KitsuneP2pTuningParams,
        signal_url: String,
    ) -> KitsuneP2pResult<(Self, MetaNetEvtRecv)> {
        let (mut evt_send, evt_recv) =
            futures::channel::mpsc::channel(tuning_params.concurrent_limit_per_thread);

        let tx4_config = tx4::DefConfig::default()
            .with_max_send_bytes(tuning_params.tx4_max_send_bytes)
            .with_max_recv_bytes(tuning_params.tx4_max_recv_bytes)
            .with_max_conn_count(tuning_params.tx4_max_conn_count)
            .with_max_conn_init(tuning_params.tx4_max_conn_init());

        let (ep_hnd, mut ep_evt) = tx4::Ep::with_config(tx4_config).await?;

        let cli_url = ep_hnd.listen(tx4::Tx4Url::new(&signal_url)?).await?;

        let ep_hnd2 = ep_hnd.clone();
        tokio::task::spawn(async move {
            while let Some(evt) = ep_evt.recv().await {
                let evt = match evt {
                    Ok(evt) => evt,
                    // TODO - FIXME - handle errors / reconnect?
                    Err(err) => panic!("{:?}", err),
                };

                match evt {
                    tx4::EpEvt::Connected { rem_cli_url } => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: rem_cli_url.to_string(),
                                con: MetaNetCon::Tx4(ep_hnd2.clone(), rem_cli_url),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tx4::EpEvt::Disconnected { rem_cli_url } => {
                        if evt_send
                            .send(MetaNetEvt::Disconnected {
                                remote_url: rem_cli_url.to_string(),
                                con: MetaNetCon::Tx4(ep_hnd2.clone(), rem_cli_url),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tx4::EpEvt::Data {
                        rem_cli_url,
                        mut data,
                        permit,
                    } => {
                        use kitsune_p2p_types::codec::Codec;
                        match wire::Wire::decode(&mut data) {
                            Ok(data) => {
                                if evt_send
                                    .send(MetaNetEvt::Notify {
                                        remote_url: rem_cli_url.to_string(),
                                        con: MetaNetCon::Tx4(ep_hnd2.clone(), rem_cli_url),
                                        data,
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::error!(?err, "decoding error");
                                // TODO - drop connection??
                            }
                        }
                    }
                    tx4::EpEvt::Demo { rem_cli_url: _ } => (),
                }
            }
        });

        Ok((MetaNet::Tx4(ep_hnd, cli_url), evt_recv))
    }

    pub fn local_addr(&self) -> KitsuneResult<String> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep) = self {
                return ep.local_addr().map(|s| s.to_string());
            }
        }

        #[cfg(feature = "tx4")]
        {
            if let MetaNet::Tx4(_ep, cli_url) = self {
                return Ok(cli_url.to_string());
            }
        }

        panic!("invalid features");
    }

    pub fn local_id(&self) -> Arc<[u8; 32]> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep) = self {
                return ep.local_cert().into();
            }
        }

        #[cfg(feature = "tx4")]
        {
            if let MetaNet::Tx4(_ep, cli_url) = self {
                if let Some(id) = cli_url.id() {
                    return Arc::new(id.0);
                }
            }
        }

        panic!("invalid features");
    }

    pub async fn close(&self, code: u32, reason: &str) {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep) = self {
                ep.close(code, reason).await;
                return;
            }
        }

        // TODO - currently no way to shutdown tx4
    }

    pub async fn get_connection(
        &self,
        remote_url: String,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<MetaNetCon> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep) = self {
                let con = ep.get_connection(remote_url, timeout).await?;
                return Ok(MetaNetCon::Tx2(con));
            }
        }

        #[cfg(feature = "tx4")]
        {
            if let MetaNet::Tx4(ep, _cli_url) = self {
                return Ok(MetaNetCon::Tx4(
                    ep.clone(),
                    tx4::Tx4Url::new(remote_url).map_err(KitsuneError::other)?,
                ));
            }
        }

        Err("invalid features".into())
    }
}
