use crate::error::{ConductorApiError, ConductorApiResult};
use crate::reconnect::{connect_with_backoff, delay_for_attempt, ReconnectConfig};
use crate::util::AbortOnDropHandle;
use crate::AdminWebsocket;
use holochain_websocket::{ConnectRequest, WebsocketConfig};
use parking_lot::RwLock;
use std::fmt::Formatter;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

/// An admin websocket that re-establishes itself after the conductor restarts.
///
/// Requests made while the connection is down fail with
/// [`ConductorApiError::Disconnected`] rather than blocking. Between a
/// connection dying and that being observed, a request can still be issued on
/// the dead socket and fail with the underlying websocket error instead; both
/// are retryable and callers need not distinguish them.
///
/// Reconnection never gives up; drop every clone of this handle to stop it.
#[derive(Clone)]
pub struct ReconnectingAdminWebsocket {
    current: Arc<RwLock<Option<AdminWebsocket>>>,
    _task: Arc<AbortOnDropHandle>,
}

impl std::fmt::Debug for ReconnectingAdminWebsocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReconnectingAdminWebsocket").finish()
    }
}

impl ReconnectingAdminWebsocket {
    /// Connects to a conductor admin interface and keeps the connection alive.
    ///
    /// The initial connection is attempted once per resolved address and fails
    /// if none of them accept, so that a typo'd port or a rejected origin is
    /// reported rather than retried silently. Use
    /// [`ReconnectingAdminWebsocket::connect_with_retry`] to wait for a
    /// conductor that has not started yet. Once this returns, connection
    /// failures are repaired instead of reported.
    pub async fn connect(
        socket_addr: impl ToSocketAddrs,
        origin: Option<String>,
        config: ReconnectConfig,
    ) -> ConductorApiResult<Self> {
        let addrs = resolve(socket_addr)?;
        let connected = connect_once(&addrs, &origin).await?;
        Ok(Self::start(addrs, origin, config, connected))
    }

    /// Connects to a conductor admin interface, waiting for it to accept.
    ///
    /// Unlike [`ReconnectingAdminWebsocket::connect`] this retries the initial
    /// connection with the same backoff used for reconnection, so it is the
    /// right choice when the conductor may not have started yet. It never
    /// gives up; bound it by wrapping the call in [`tokio::time::timeout`],
    /// which works because the retry loop is cancel safe.
    pub async fn connect_with_retry(
        socket_addr: impl ToSocketAddrs,
        origin: Option<String>,
        config: ReconnectConfig,
    ) -> ConductorApiResult<Self> {
        let addrs = resolve(socket_addr)?;
        let connected = connect_with_backoff("holochain_client::admin", &config, || {
            connect_once(&addrs, &origin)
        })
        .await;
        Ok(Self::start(addrs, origin, config, connected))
    }

    fn start(
        addrs: Vec<SocketAddr>,
        origin: Option<String>,
        config: ReconnectConfig,
        connected: (AdminWebsocket, crate::util::ClosedNotify),
    ) -> Self {
        let (admin_ws, closed) = connected;

        let current = Arc::new(RwLock::new(Some(admin_ws)));

        let task = tokio::task::spawn({
            let current = current.clone();
            async move {
                let mut closed = closed;
                let mut flaps: u32 = 0;
                loop {
                    let connected_at = std::time::Instant::now();
                    closed.closed().await;
                    current.write().take();

                    // A connection accepted and then dropped straight away
                    // would otherwise reconnect with no delay, because the
                    // backoff counter only advances on failed connects. Count
                    // a short-lived connection as a failed attempt.
                    if connected_at.elapsed() < config.initial_delay {
                        let delay = delay_for_attempt(flaps, &config);
                        tracing::warn!(
                            target: "holochain_client::admin",
                            flaps,
                            ?delay,
                            "connection closed shortly after connecting, backing off before reconnecting"
                        );
                        flaps = flaps.saturating_add(1);
                        tokio::time::sleep(delay).await;
                    } else {
                        flaps = 0;
                    }

                    let (admin_ws, next_closed) =
                        connect_with_backoff("holochain_client::admin", &config, || {
                            connect_once(&addrs, &origin)
                        })
                        .await;

                    current.write().replace(admin_ws);
                    closed = next_closed;
                }
            }
        });

        Self {
            current,
            _task: Arc::new(AbortOnDropHandle::new(task.abort_handle())),
        }
    }

    /// Returns the live admin websocket.
    ///
    /// # Errors
    ///
    /// Returns [`ConductorApiError::Disconnected`] while the connection is
    /// being re-established.
    pub fn current(&self) -> ConductorApiResult<AdminWebsocket> {
        self.current
            .read()
            .clone()
            .ok_or(ConductorApiError::Disconnected)
    }
}

fn resolve(socket_addr: impl ToSocketAddrs) -> ConductorApiResult<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = socket_addr.to_socket_addrs()?.collect();
    if addrs.is_empty() {
        return Err(ConductorApiError::NoAddressesResolved);
    }
    Ok(addrs)
}

async fn connect_once(
    addrs: &[SocketAddr],
    origin: &Option<String>,
) -> ConductorApiResult<(AdminWebsocket, crate::util::ClosedNotify)> {
    let websocket_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);

    let mut last_err = None;
    for addr in addrs {
        let request: ConnectRequest = match origin {
            Some(o) => Into::<ConnectRequest>::into(*addr).try_set_header("Origin", o.as_str())?,
            None => (*addr).into(),
        };

        match AdminWebsocket::connect_with_notify(request, websocket_config.clone()).await {
            Ok(connected) => return Ok(connected),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or(ConductorApiError::NoAddressesResolved))
}
