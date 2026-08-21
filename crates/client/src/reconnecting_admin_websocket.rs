use crate::error::{ConductorApiError, ConductorApiResult};
use crate::reconnect::{connect_with_backoff, delay_for_attempt, delegate, ReconnectConfig};
use crate::util::AbortOnDropHandle;
use crate::{AdminWebsocket, AuthorizeSigningCredentialsPayload, EnableAppResponse};
use holo_hash::{ActionHash, DnaHash};
use holochain_conductor_api::{
    AdminInterfaceConfig, AppAuthenticationToken, AppAuthenticationTokenIssued, AppInfo,
    AppInterfaceInfo, AppStatusFilter, DhtOpsCursor, FullStateDump,
    IssueAppAuthenticationTokenPayload, OpTimingsCursor, OpTimingsDump, PeerMetaInfo,
    SourceChainCursor, StorageInfo,
};
use holochain_types::dna::AgentPubKey;
use holochain_types::network::HolochainTransportStats;
use holochain_types::prelude::{
    AppCapGrantInfo, CellId, DeleteCloneCellPayload, InstallAppPayload, UpdateCoordinatorsPayload,
};
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::{ConnectRequest, WebsocketConfig};
use holochain_zome_types::prelude::{DnaDef, GrantZomeCallCapabilityPayload};
use kitsune2_api::Url;
use parking_lot::RwLock;
use std::collections::BTreeMap;
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
    /// The returned socket is a snapshot of the connection as it stands, and a
    /// reconnect replaces it. Call this once per request and drop the result;
    /// a stored socket stops working at the next reconnect and reports raw
    /// websocket errors instead of [`ConductorApiError::Disconnected`].
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

    delegate!(
        /// Issues an app authentication token for an app.
        issue_app_auth_token(
            payload: IssueAppAuthenticationTokenPayload,
        ) -> AppAuthenticationTokenIssued
    );
    delegate!(
        /// Revokes a previously issued app authentication token.
        revoke_app_authentication_token(token: AppAuthenticationToken) -> ()
    );
    delegate!(
        /// Generates a new agent public key in the keystore.
        generate_agent_pub_key() -> AgentPubKey
    );
    delegate!(
        /// Adds admin interfaces to the conductor.
        add_admin_interfaces(configs: Vec<AdminInterfaceConfig>) -> ()
    );
    delegate!(
        /// Lists the app interfaces attached to the conductor.
        list_app_interfaces() -> Vec<AppInterfaceInfo>
    );
    delegate!(
        /// Attaches an app interface and returns the port it listens on.
        attach_app_interface(
            port: u16,
            danger_bind_addr: Option<String>,
            allowed_origins: AllowedOrigins,
            installed_app_id: Option<String>,
        ) -> u16
    );
    delegate!(
        /// Lists the installed apps, optionally filtered by status.
        list_apps(status_filter: Option<AppStatusFilter>) -> Vec<AppInfo>
    );
    delegate!(
        /// Installs an app.
        install_app(payload: InstallAppPayload) -> AppInfo
    );
    delegate!(
        /// Uninstalls an app.
        uninstall_app(installed_app_id: String, force: bool) -> ()
    );
    delegate!(
        /// Lists the DNAs registered with the conductor.
        list_dnas() -> Vec<DnaHash>
    );
    delegate!(
        /// Enables an app.
        enable_app(installed_app_id: String) -> EnableAppResponse
    );
    delegate!(
        /// Disables an app.
        disable_app(installed_app_id: String) -> ()
    );
    delegate!(
        /// Lists the cell ids the conductor is running.
        list_cell_ids() -> Vec<CellId>
    );
    delegate!(
        /// Gets a cell's DNA definition.
        get_dna_definition(cell_id: CellId) -> DnaDef
    );
    delegate!(
        /// Grants a zome call capability on a cell.
        grant_zome_call_capability(payload: GrantZomeCallCapabilityPayload) -> ActionHash
    );
    delegate!(
        /// Lists an app's capability grants.
        list_capability_grants(
            installed_app_id: String,
            include_revoked: bool,
        ) -> AppCapGrantInfo
    );
    delegate!(
        /// Revokes a zome call capability on a cell.
        revoke_zome_call_capability(cell_id: CellId, action_hash: ActionHash) -> ()
    );
    delegate!(
        /// Deletes a disabled clone cell.
        delete_clone_cell(payload: DeleteCloneCellPayload) -> ()
    );
    delegate!(
        /// Reports the conductor's storage usage.
        storage_info() -> StorageInfo
    );
    delegate!(
        /// Dumps network transport statistics.
        dump_network_stats() -> HolochainTransportStats
    );
    delegate!(
        /// Dumps one page of a cell's source-chain state.
        dump_state(
            cell_id: CellId,
            source_chain_cursor: Option<SourceChainCursor>,
            limit: Option<u32>,
        ) -> String
    );
    delegate!(
        /// Dumps the conductor's state.
        dump_conductor_state() -> String
    );
    delegate!(
        /// Dumps one page of a cell's full state.
        dump_full_state(
            cell_id: CellId,
            dht_ops_cursor: Option<DhtOpsCursor>,
            limit: Option<u32>,
        ) -> FullStateDump
    );
    delegate!(
        /// Dumps one page of a DNA's DHT-op lifecycle timings.
        dump_op_timings(
            dna_hash: DnaHash,
            cursor: Option<OpTimingsCursor>,
            limit: Option<u32>,
        ) -> OpTimingsDump
    );
    delegate!(
        /// Dumps network metrics.
        dump_network_metrics(
            dna_hash: Option<DnaHash>,
            include_dht_summary: bool,
        ) -> std::collections::HashMap<DnaHash, holochain_types::network::Kitsune2NetworkMetrics>
    );
    delegate!(
        /// Updates an app's coordinator zomes.
        update_coordinators(update_coordinators_payload: UpdateCoordinatorsPayload) -> ()
    );
    delegate!(
        /// Lists known peers, optionally restricted to some DNAs.
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
    delegate!(
        /// Grants a capability to a freshly generated signing keypair.
        authorize_signing_credentials(
            request: AuthorizeSigningCredentialsPayload,
        ) -> crate::signing::client_signing::SigningCredentials
    );
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
