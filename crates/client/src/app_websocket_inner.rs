use crate::error::{ConductorApiError, ConductorApiResult};
use crate::util::AbortOnDropHandle;
use event_emitter_rs::EventEmitter;
use holochain_conductor_api::{
    AppAuthenticationRequest, AppAuthenticationToken, AppInfo, AppRequest, AppResponse,
};
use holochain_types::signal::Signal;
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use std::fmt::Formatter;
use std::{net::ToSocketAddrs, sync::Arc};
use tokio::sync::Mutex;

/// The core functionality for an app websocket.
#[derive(Clone)]
pub(crate) struct AppWebsocketInner {
    tx: WebsocketSender,
    event_emitter: Arc<Mutex<EventEmitter>>,
    _abort_handle: Arc<AbortOnDropHandle>,
}

impl std::fmt::Debug for AppWebsocketInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppWebsocketInner").finish()
    }
}

impl AppWebsocketInner {
    /// Connect to a Conductor API app websocket.
    pub(crate) async fn connect(
        socket_addr: impl ToSocketAddrs,
        origin: Option<String>,
    ) -> ConductorApiResult<Self> {
        let websocket_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);

        Self::connect_with_config(socket_addr, websocket_config, origin).await
    }

    /// Connect to a Conductor API app websocket with a custom [WebsocketConfig].
    pub async fn connect_with_config(
        socket_addr: impl ToSocketAddrs,
        websocket_config: Arc<WebsocketConfig>,
        origin: Option<String>,
    ) -> ConductorApiResult<Self> {
        let mut last_err = None;
        for addr in socket_addr.to_socket_addrs()? {
            let request: ConnectRequest = if let Some(o) = &origin {
                Into::<ConnectRequest>::into(addr).try_set_header("Origin", o.as_str())?
            } else {
                addr.into()
            };

            match Self::connect_with_config_and_request(websocket_config.clone(), request).await {
                Ok(app_ws) => return Ok(app_ws),
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(
                "No addresses resolved".to_string(),
            ))
        }))
    }

    /// Connect to a Conductor API app websocket with a custom [WebsocketConfig] and [ConnectRequest].
    pub async fn connect_with_config_and_request(
        websocket_config: Arc<WebsocketConfig>,
        request: ConnectRequest,
    ) -> ConductorApiResult<Self> {
        let (tx, mut rx) = connect(websocket_config, request).await?;

        let event_emitter = EventEmitter::new();
        let mutex = Arc::new(Mutex::new(event_emitter));

        let poll_handle = tokio::task::spawn({
            let mutex = mutex.clone();
            async move {
                while let Ok(msg) = rx.recv::<AppResponse>().await {
                    if let holochain_websocket::ReceiveMessage::Signal(signal_bytes) = msg {
                        let mut event_emitter = mutex.lock().await;
                        event_emitter.emit("signal", signal_bytes);
                    }
                }
            }
        });

        Ok(Self {
            tx,
            event_emitter: mutex,
            _abort_handle: Arc::new(AbortOnDropHandle::new(poll_handle.abort_handle())),
        })
    }

    pub(crate) async fn on_signal<F: Fn(Signal) + 'static + Sync + Send>(
        &self,
        handler: F,
    ) -> String {
        let mut event_emitter = self.event_emitter.lock().await;
        event_emitter.on("signal", move |signal_bytes| {
            let signal: Signal =
                Signal::try_from_vec(signal_bytes).expect("Failed to deserialize signal");
            handler(signal);
        })
    }

    pub(crate) async fn app_info(&self) -> ConductorApiResult<Option<AppInfo>> {
        let response = self.send(AppRequest::AppInfo).await?;
        match response {
            AppResponse::AppInfo(app_info) => Ok(app_info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub(crate) async fn authenticate(
        &self,
        token: AppAuthenticationToken,
    ) -> ConductorApiResult<()> {
        self.tx
            .authenticate(AppAuthenticationRequest { token })
            .await
            .map_err(ConductorApiError::WebsocketError)
    }

    /// Sends a request using the connection-level default timeout.
    pub(crate) async fn send(&self, msg: AppRequest) -> ConductorApiResult<AppResponse> {
        self.send_with_timeout(msg, None).await
    }

    /// Sends a request with an optional per-call timeout override.
    ///
    /// When `timeout` is `None`, the connection-level default is used.
    pub(crate) async fn send_with_timeout(
        &self,
        msg: AppRequest,
        timeout: Option<std::time::Duration>,
    ) -> ConductorApiResult<AppResponse> {
        let response = match timeout {
            Some(t) => self.tx.request_timeout(msg, t).await,
            None => self.tx.request(msg).await,
        }
        .map_err(ConductorApiError::WebsocketError)?;

        match response {
            AppResponse::Error(error) => Err(ConductorApiError::ExternalApiWireError(error)),
            _ => Ok(response),
        }
    }
}
