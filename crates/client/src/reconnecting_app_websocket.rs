use crate::app_connect::discover_app_interface_port;
use crate::error::{ConductorApiError, ConductorApiResult};
use crate::reconnect::{connect_with_backoff, delay_for_attempt, ReconnectConfig};
use crate::signal_stream::{signal_stream, SignalEvent, SignalStream, SIGNAL_CHANNEL_CAPACITY};
use crate::util::{AbortOnDropHandle, ClosedNotify};
use crate::{AdminWebsocket, AppWebsocket, DynAgentSigner, ReconnectingAdminWebsocket};
use holo_hash::DnaHash;
use holochain_conductor_api::{
    AppInfo, IssueAppAuthenticationTokenPayload, OpTimingsCursor, OpTimingsDump, PeerMetaInfo,
    ZomeCallParamsSigned,
};
use holochain_types::app::{
    CreateCloneCellPayload, DisableCloneCellPayload, EnableCloneCellPayload, InstalledAppId,
    MemproofMap,
};
use holochain_websocket::{ConnectRequest, WebsocketConfig};
use holochain_zome_types::clone::ClonedCell;
use holochain_zome_types::prelude::{ExternIO, FunctionName, ZomeName};
use kitsune2_api::Url;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Builds a [`ReconnectingAppWebsocket`].
pub struct ReconnectingAppWebsocketBuilder {
    admin_addr: SocketAddr,
    installed_app_id: InstalledAppId,
    signer: DynAgentSigner,
    origin: Option<String>,
    reconnect_config: ReconnectConfig,
}

impl ReconnectingAppWebsocketBuilder {
    /// Sets the origin sent when connecting, which must be admitted by the
    /// app interface's allowed origins.
    ///
    /// The same origin is sent when connecting to the admin interface, so it
    /// has to be admitted by both interfaces. There is no way to give the two
    /// different origins.
    pub fn origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Sets the backoff applied between reconnect attempts.
    pub fn reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }

    /// Establishes the connection, failing if the conductor does not accept.
    ///
    /// # Errors
    ///
    /// Returns an error if the conductor is unreachable, if no app interface
    /// accepts this app from this origin, or if the app is not installed. Use
    /// [`ReconnectingAppWebsocketBuilder::connect_with_retry`] to wait for a
    /// conductor that has not started yet. After this returns successfully,
    /// connection failures are repaired instead of reported.
    pub async fn connect(self) -> ConductorApiResult<ReconnectingAppWebsocket> {
        let admin_ws = ReconnectingAdminWebsocket::connect(
            self.admin_addr,
            self.origin.clone(),
            self.reconnect_config.clone(),
        )
        .await?;

        let connected = connect_app(
            &admin_ws,
            self.admin_addr,
            &self.installed_app_id,
            self.origin.as_deref(),
            self.signer.clone(),
        )
        .await?;

        Ok(self.start(admin_ws, connected).await)
    }

    /// Establishes the connection, waiting for the conductor to accept.
    ///
    /// Retries with the same backoff used for reconnection, so it is the right
    /// choice when the conductor may not have started yet, or when its app
    /// interface or the app itself is still being set up. It never gives up;
    /// bound it by wrapping the call in [`tokio::time::timeout`], which works
    /// because the retry loop is cancel safe.
    ///
    /// A permanent misconfiguration, such as an app id that is not installed
    /// or an origin no app interface admits, retries exactly as a conductor
    /// that is merely down does. To tell the two apart, connect with
    /// [`ReconnectingAppWebsocketBuilder::connect`] or bound this call with a
    /// timeout.
    pub async fn connect_with_retry(self) -> ConductorApiResult<ReconnectingAppWebsocket> {
        let admin_ws = ReconnectingAdminWebsocket::connect_with_retry(
            self.admin_addr,
            self.origin.clone(),
            self.reconnect_config.clone(),
        )
        .await?;

        let connected =
            connect_with_backoff("holochain_client::app", &self.reconnect_config, || {
                connect_app(
                    &admin_ws,
                    self.admin_addr,
                    &self.installed_app_id,
                    self.origin.as_deref(),
                    self.signer.clone(),
                )
            })
            .await;

        Ok(self.start(admin_ws, connected).await)
    }

    async fn start(
        self,
        admin_ws: ReconnectingAdminWebsocket,
        connected: (AppWebsocket, ClosedNotify),
    ) -> ReconnectingAppWebsocket {
        let (app_ws, closed) = connected;

        let (signal_tx, _) = broadcast::channel(SIGNAL_CHANNEL_CAPACITY);

        forward_signals(&app_ws, signal_tx.clone()).await;

        let current = Arc::new(RwLock::new(Some(app_ws)));

        let task = tokio::task::spawn({
            let current = current.clone();
            let signal_tx = signal_tx.clone();
            let admin_ws = admin_ws.clone();
            let installed_app_id = self.installed_app_id.clone();
            let admin_addr = self.admin_addr;
            let origin = self.origin.clone();
            let signer = self.signer.clone();
            let config = self.reconnect_config.clone();
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
                            target: "holochain_client::app",
                            flaps,
                            ?delay,
                            "connection closed shortly after connecting, backing off before reconnecting"
                        );
                        flaps = flaps.saturating_add(1);
                        tokio::time::sleep(delay).await;
                    } else {
                        flaps = 0;
                    }

                    let (app_ws, next_closed) =
                        connect_with_backoff("holochain_client::app", &config, || {
                            connect_app(
                                &admin_ws,
                                admin_addr,
                                &installed_app_id,
                                origin.as_deref(),
                                signer.clone(),
                            )
                        })
                        .await;

                    // The connection is published before the gap is
                    // reported, so a consumer woken by `Interrupted` re-syncs
                    // against a live connection. A signal arriving before the
                    // forwarder is registered is dropped, which is what the
                    // `Interrupted` that precedes it tells the consumer to
                    // recover.
                    current.write().replace(app_ws.clone());
                    let _ = signal_tx.send(SignalEvent::Interrupted);
                    forward_signals(&app_ws, signal_tx.clone()).await;

                    closed = next_closed;
                }
            }
        });

        ReconnectingAppWebsocket {
            current,
            signal_tx,
            _admin_ws: admin_ws,
            _task: Arc::new(AbortOnDropHandle::new(task.abort_handle())),
        }
    }
}

/// An app websocket that re-establishes itself after the conductor restarts.
///
/// Between a connection dying and that being observed, a request can still be
/// issued on the dead socket and fail with the underlying websocket error
/// rather than [`ConductorApiError::Disconnected`]; both are retryable and
/// callers need not distinguish them.
///
/// Requests made while the connection is down fail with
/// [`ConductorApiError::Disconnected`] and are never retried automatically,
/// because re-signing a zome call mints a fresh nonce and would risk writing
/// to the source chain twice. Signal subscriptions taken from
/// [`ReconnectingAppWebsocket::signals`] survive reconnects.
///
/// Reconnection never gives up. Drop every clone of this handle to stop it.
#[derive(Clone)]
pub struct ReconnectingAppWebsocket {
    current: Arc<RwLock<Option<AppWebsocket>>>,
    signal_tx: broadcast::Sender<SignalEvent>,
    _admin_ws: ReconnectingAdminWebsocket,
    _task: Arc<AbortOnDropHandle>,
}

impl std::fmt::Debug for ReconnectingAppWebsocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReconnectingAppWebsocket").finish()
    }
}

macro_rules! delegate {
    ($(#[$meta:meta])* $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty) => {
        $(#[$meta])*
        pub async fn $name(&self, $($arg: $ty),*) -> ConductorApiResult<$ret> {
            self.current()?.$name($($arg),*).await
        }
    };
}

impl ReconnectingAppWebsocket {
    /// Starts building a connection to an app.
    ///
    /// The connection is identified by the conductor's admin address and the
    /// installed app id rather than by an app interface port, because the port
    /// an app interface listens on is not stable across a conductor restart.
    pub fn builder(
        admin_addr: SocketAddr,
        installed_app_id: InstalledAppId,
        signer: DynAgentSigner,
    ) -> ReconnectingAppWebsocketBuilder {
        ReconnectingAppWebsocketBuilder {
            admin_addr,
            installed_app_id,
            signer,
            origin: None,
            reconnect_config: ReconnectConfig::default(),
        }
    }

    /// Subscribes to the app's signals.
    ///
    /// Each call returns an independent subscription that receives every signal
    /// from this point on, and keeps doing so across reconnects. Signals
    /// emitted while the connection was down are reported as
    /// [`SignalEvent::Interrupted`] rather than delivered, because Holochain
    /// does not replay them.
    pub fn signals(&self) -> SignalStream {
        signal_stream(self.signal_tx.subscribe())
    }

    /// Returns the live app websocket.
    ///
    /// # Errors
    ///
    /// Returns [`ConductorApiError::Disconnected`] while the connection is
    /// being re-established.
    pub fn current(&self) -> ConductorApiResult<AppWebsocket> {
        self.current
            .read()
            .clone()
            .ok_or(ConductorApiError::Disconnected)
    }

    delegate!(
        /// Calls a zome function using the connection-level default timeout.
        call_zome(
            target: crate::ZomeCallTarget,
            zome_name: ZomeName,
            fn_name: FunctionName,
            payload: ExternIO,
        ) -> ExternIO
    );
    delegate!(
        /// Calls a zome function with per-call options.
        call_zome_with_options(
            target: crate::ZomeCallTarget,
            zome_name: ZomeName,
            fn_name: FunctionName,
            payload: ExternIO,
            options: crate::CallZomeOptions,
        ) -> ExternIO
    );
    delegate!(
        /// Sends a pre-signed zome call.
        signed_call_zome(signed_params: ZomeCallParamsSigned) -> ExternIO
    );
    delegate!(
        /// Sends a pre-signed zome call with per-call options.
        signed_call_zome_with_options(
            signed_params: ZomeCallParamsSigned,
            options: crate::CallZomeOptions,
        ) -> ExternIO
    );
    delegate!(
        /// Gets the app's current info.
        app_info() -> Option<AppInfo>
    );
    delegate!(
        /// Enables the app.
        enable_app() -> ()
    );
    delegate!(
        /// Provides membrane proofs for a deferred-memproof app.
        provide_memproofs(memproofs: MemproofMap) -> ()
    );
    delegate!(
        /// Creates a clone cell.
        create_clone_cell(msg: CreateCloneCellPayload) -> ClonedCell
    );
    delegate!(
        /// Disables a clone cell.
        disable_clone_cell(payload: DisableCloneCellPayload) -> ()
    );
    delegate!(
        /// Enables a clone cell.
        enable_clone_cell(payload: EnableCloneCellPayload) -> ClonedCell
    );
    delegate!(
        /// Lists the host functions available to wasm.
        list_wasm_host_functions() -> Vec<String>
    );
    delegate!(
        /// Dumps network transport statistics.
        dump_network_stats() -> holochain_types::network::HolochainTransportStats
    );
    delegate!(
        /// Dumps network metrics.
        dump_network_metrics(
            dna_hash: Option<DnaHash>,
            include_dht_summary: bool,
        ) -> std::collections::HashMap<DnaHash, holochain_types::network::Kitsune2NetworkMetrics>
    );
    delegate!(
        /// Dumps one page of DHT-op lifecycle timings.
        dump_op_timings(
            dna_hash: DnaHash,
            cursor: Option<OpTimingsCursor>,
            limit: Option<u32>,
        ) -> OpTimingsDump
    );
    delegate!(
        /// Lists connected peers for the app's DNAs.
        agent_info(dna_hashes: Option<Vec<DnaHash>>) -> Vec<String>
    );
    delegate!(
        /// Adds signed agent info to the conductor's peer store.
        add_agent_info(agent_infos: Vec<String>) -> ()
    );
    delegate!(
        /// Reads the peer meta store for an agent at a URL.
        peer_meta_info(
            url: Url,
            dna_hashes: Option<Vec<DnaHash>>,
        ) -> BTreeMap<DnaHash, BTreeMap<String, PeerMetaInfo>>
    );
}

async fn forward_signals(app_ws: &AppWebsocket, signal_tx: broadcast::Sender<SignalEvent>) {
    app_ws
        .on_signal(move |signal| {
            let _ = signal_tx.send(SignalEvent::Signal(signal));
        })
        .await;
}

async fn connect_app(
    admin_ws: &ReconnectingAdminWebsocket,
    admin_addr: SocketAddr,
    installed_app_id: &InstalledAppId,
    origin: Option<&str>,
    signer: DynAgentSigner,
) -> ConductorApiResult<(AppWebsocket, ClosedNotify)> {
    let admin: AdminWebsocket = admin_ws.current()?;

    let port = discover_app_interface_port(&admin, installed_app_id, origin).await?;

    let token = admin
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            installed_app_id.clone(),
        ))
        .await?
        .token;

    // The app interface listens on the same host as the admin interface, so
    // reuse its address and take only the discovered port.
    let addr = SocketAddr::new(admin_addr.ip(), port);
    let request: ConnectRequest = match origin {
        Some(o) => Into::<ConnectRequest>::into(addr).try_set_header("Origin", o)?,
        None => addr.into(),
    };

    AppWebsocket::connect_with_notify(
        request,
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        token,
        signer,
    )
    .await
}
