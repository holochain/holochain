use crate::app_websocket_inner::AppWebsocketInner;
use crate::signing::DynAgentSigner;
use crate::{signing::sign_zome_call, ConductorApiError, ConductorApiResult};
use anyhow::{anyhow, Result};
use holo_hash::AgentPubKey;
use holochain_conductor_api::{
    AppAuthenticationToken, AppInfo, AppRequest, AppResponse, CellInfo, ProvisionedCell,
    ZomeCallParamsSigned,
};
use holochain_nonce::fresh_nonce;
use holochain_types::app::{
    CreateCloneCellPayload, DisableCloneCellPayload, EnableCloneCellPayload, MemproofMap,
};
use holochain_types::prelude::{CloneId, Signal};
use holochain_websocket::{ConnectRequest, WebsocketConfig};
use holochain_zome_types::{
    clone::ClonedCell,
    prelude::{CellId, ExternIO, FunctionName, RoleName, Timestamp, ZomeCallParams, ZomeName},
};
use std::fmt::Formatter;
use std::net::ToSocketAddrs;
use std::sync::Arc;

/// A websocket connection to a Holochain app running in a Conductor.
#[derive(Clone)]
pub struct AppWebsocket {
    pub my_pub_key: AgentPubKey,
    inner: AppWebsocketInner,
    app_info: AppInfo,
    signer: DynAgentSigner,
}

impl std::fmt::Debug for AppWebsocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppWebsocket")
            .field("my_pub_key", &self.my_pub_key)
            .field("inner", &self.inner)
            .field("app_info", &self.app_info)
            .finish()
    }
}

impl AppWebsocket {
    /// Connect to a Conductor API app websocket.
    ///
    /// `socket_addr` is a websocket address that implements [ToSocketAddr](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html#tymethod.to_socket_addrs).
    ///
    /// `token` is an [AppAuthenticationToken] that is issued by the admin interface using [issue_app_auth_token](crate::AdminWebsocket::issue_app_auth_token).
    /// Tokens are issued for a specific installed app, so this websocket will only be able to interact with that app.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[tokio::main]
    /// # async fn main() {
    /// use std::net::Ipv4Addr;
    /// use holochain_client::{AdminWebsocket, AppWebsocket, ClientAgentSigner};
    ///
    /// let mut admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, 30_000)).await.unwrap();
    ///
    /// let app_id = "test-app".to_string();
    /// let issued = admin_ws.issue_app_auth_token(app_id.clone().into()).await.unwrap();
    /// let signer = ClientAgentSigner::default();
    /// let app_ws = AppWebsocket::connect((Ipv4Addr::LOCALHOST, 30_001), issued.token, signer.into(), None).await.unwrap();
    /// # }
    /// ```
    ///
    /// As string: `"localhost:30000"`
    ///
    /// As tuple: `([127.0.0.1], 30000)`
    pub async fn connect(
        socket_addr: impl ToSocketAddrs,
        token: AppAuthenticationToken,
        signer: DynAgentSigner,
        origin: Option<String>,
    ) -> ConductorApiResult<Self> {
        let app_ws = AppWebsocketInner::connect(socket_addr, origin).await?;
        Self::post_connect(app_ws, token, signer).await
    }

    /// Connect to a Conductor API app websocket with a custom [WebsocketConfig].
    ///
    /// You need to use this constructor if you want to set a lower timeout than the default.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[tokio::main]
    /// # async fn main() {
    /// use std::net::Ipv4Addr;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use holochain_client::{AdminWebsocket, AppWebsocket, AllowedOrigins, WebsocketConfig, ClientAgentSigner};
    ///
    /// let mut admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, 30_000)).await.unwrap();
    ///
    /// let app_id = "test-app".to_string();
    /// let issued = admin_ws.issue_app_auth_token(app_id.clone().into()).await.unwrap();
    ///
    /// // Create a client config from the default and sets a timeout that is lower than the default
    /// let mut client_config = WebsocketConfig::CLIENT_DEFAULT;
    /// client_config.default_request_timeout = Duration::from_secs(10);
    ///
    /// let client_config = Arc::new(client_config);
    ///
    /// let signer = ClientAgentSigner::default();
    /// let app_ws = AppWebsocket::connect_with_config((Ipv4Addr::LOCALHOST, 30_001), client_config, issued.token, signer.into(), None).await.unwrap();
    /// # }
    /// ```
    pub async fn connect_with_config(
        socket_addr: impl ToSocketAddrs,
        websocket_config: Arc<WebsocketConfig>,
        token: AppAuthenticationToken,
        signer: DynAgentSigner,
        origin: Option<String>,
    ) -> ConductorApiResult<Self> {
        let app_ws =
            AppWebsocketInner::connect_with_config(socket_addr, websocket_config, origin).await?;
        Self::post_connect(app_ws, token, signer).await
    }

    /// Connect to a Conductor API app websocket with a custom [WebsocketConfig] and [ConnectRequest].
    ///
    /// This is a low-level constructor that allows you to pass a custom [ConnectRequest] to the
    /// websocket connection. You should use this if you need to set custom connection headers.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[tokio::main]
    /// # async fn main() {
    /// use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use holochain_client::{
    ///     AdminWebsocket, AppWebsocket, AllowedOrigins, WebsocketConfig,
    ///     ConnectRequest, ClientAgentSigner, AgentSigner, DynAgentSigner
    /// };
    ///
    /// let mut admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, 30_000)).await.unwrap();
    ///
    /// let app_id = "test-app".to_string();
    /// let issued = admin_ws.issue_app_auth_token(app_id.clone().into()).await.unwrap();
    ///
    /// // Use the default client config
    /// let mut client_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);
    ///
    /// let signer: DynAgentSigner = ClientAgentSigner::default().into();
    ///
    /// // Attempt to connect to Holochain on one of these interfaces on port 30,001
    /// let connect_to = vec![
    ///     SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 30_001),
    ///     SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 30_001),
    /// ];
    /// for addr in connect_to {
    ///     match AppWebsocket::connect_with_request_and_config(request, client_config.clone(), issued.token.clone(), signer.clone(), None).await {
    ///         Ok(admin_ws) => {
    ///             println!("Connected to {:?}", addr);
    ///             break;
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Failed to connect to {:?}: {}", addr, e);
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn connect_with_request_and_config(
        request: ConnectRequest,
        websocket_config: Arc<WebsocketConfig>,
        token: AppAuthenticationToken,
        signer: DynAgentSigner,
    ) -> ConductorApiResult<Self> {
        let app_ws =
            AppWebsocketInner::connect_with_config_and_request(websocket_config, request).await?;
        Self::post_connect(app_ws, token, signer).await
    }

    async fn post_connect(
        inner: AppWebsocketInner,
        token: AppAuthenticationToken,
        signer: DynAgentSigner,
    ) -> ConductorApiResult<Self> {
        inner.authenticate(token).await?;

        let app_info = inner
            .app_info()
            .await?
            .ok_or(ConductorApiError::AppNotFound)?;

        Ok(AppWebsocket {
            my_pub_key: app_info.agent_pub_key.clone(),
            inner,
            app_info,
            signer,
        })
    }

    pub async fn on_signal<F: Fn(Signal) + 'static + Sync + Send>(&self, handler: F) -> String {
        let app_info = self.app_info.clone();
        self.inner
            .on_signal(move |signal| match signal.clone() {
                Signal::App {
                    cell_id,
                    zome_name: _,
                    signal: _,
                } => {
                    if app_info.cell_info.values().any(|cells| {
                        cells.iter().any(|cell_info| match cell_info {
                            CellInfo::Provisioned(cell) => cell.cell_id.eq(&cell_id),
                            CellInfo::Cloned(cell) => cell.cell_id.eq(&cell_id),
                            _ => false,
                        })
                    }) {
                        handler(signal);
                    }
                }
                Signal::System(_) => handler(signal),
            })
            .await
    }

    pub async fn app_info(&self) -> ConductorApiResult<Option<AppInfo>> {
        self.inner.app_info().await
    }

    /// Get the cached app info held by this websocket.
    ///
    /// In order to speed up internal operations, the app info is cached by the websocket after
    /// connection and refreshed as required. You cannot control the cache lifetime, but you can
    /// use the value and fallback to [AppWebsocket::app_info] if you need to ensure you have the
    /// latest info.
    pub fn cached_app_info(&self) -> &AppInfo {
        &self.app_info
    }

    pub async fn call_zome(
        &self,
        target: ZomeCallTarget,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: ExternIO,
    ) -> ConductorApiResult<ExternIO> {
        let cell_id = match target {
            ZomeCallTarget::CellId(cell_id) => cell_id,
            ZomeCallTarget::RoleName(role_name) => self.get_cell_id_from_role_name(&role_name)?,
            ZomeCallTarget::CloneId(clone_id) => self.get_cell_id_from_role_name(&clone_id.0)?,
        };

        let (nonce, expires_at) =
            fresh_nonce(Timestamp::now()).map_err(ConductorApiError::FreshNonceError)?;

        let params = ZomeCallParams {
            provenance: self.signer.get_provenance(&cell_id).ok_or(
                ConductorApiError::SignZomeCallError("Provenance not found".to_string()),
            )?,
            cap_secret: self.signer.get_cap_secret(&cell_id),
            cell_id: cell_id.clone(),
            zome_name,
            fn_name,
            payload,
            expires_at,
            nonce,
        };
        let signed_zome_call = sign_zome_call(params, self.signer.clone())
            .await
            .map_err(|e| ConductorApiError::SignZomeCallError(e.to_string()))?;

        self.signed_call_zome(signed_zome_call).await
    }

    pub async fn signed_call_zome(
        &self,
        signed_params: ZomeCallParamsSigned,
    ) -> ConductorApiResult<ExternIO> {
        let app_request = AppRequest::CallZome(Box::new(signed_params));
        let response = self.inner.send(app_request).await?;

        match response {
            AppResponse::ZomeCalled(result) => Ok(*result),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn provide_memproofs(&self, memproofs: MemproofMap) -> ConductorApiResult<()> {
        let app_request = AppRequest::ProvideMemproofs(memproofs);
        let response = self.inner.send(app_request).await?;
        match response {
            AppResponse::Ok => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn enable_app(&self) -> ConductorApiResult<()> {
        let app_request = AppRequest::EnableApp;
        let response = self.inner.send(app_request).await?;
        match response {
            AppResponse::Ok => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn create_clone_cell(
        &self,
        msg: CreateCloneCellPayload,
    ) -> ConductorApiResult<ClonedCell> {
        let app_request = AppRequest::CreateCloneCell(Box::new(msg));
        let response = self.inner.send(app_request).await?;
        match response {
            AppResponse::CloneCellCreated(clone_cell) => Ok(clone_cell),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn disable_clone_cell(
        &self,
        payload: DisableCloneCellPayload,
    ) -> ConductorApiResult<()> {
        let app_request = AppRequest::DisableCloneCell(Box::new(payload));
        let response = self.inner.send(app_request).await?;
        match response {
            AppResponse::CloneCellDisabled => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn enable_clone_cell(
        &self,
        payload: EnableCloneCellPayload,
    ) -> ConductorApiResult<ClonedCell> {
        let msg = AppRequest::EnableCloneCell(Box::new(payload));
        let response = self.inner.send(msg).await?;
        match response {
            AppResponse::CloneCellEnabled(enabled_cell) => Ok(enabled_cell),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn list_wasm_host_functions(&self) -> ConductorApiResult<Vec<String>> {
        let msg = AppRequest::ListWasmHostFunctions;
        let response = self.inner.send(msg).await?;
        match response {
            AppResponse::ListWasmHostFunctions(functions) => Ok(functions),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    /// Gets a new copy of the [AppInfo] for the app this agent is connected to.
    ///
    /// This is useful if you have made changes to the app, such as creating new clone cells, and need to refresh the app info.
    pub async fn refresh_app_info(&mut self) -> Result<()> {
        self.app_info = self
            .app_info()
            .await
            .map_err(|err| anyhow!("Error fetching app_info {err:?}"))?
            .ok_or(anyhow!("App doesn't exist"))?;

        Ok(())
    }

    fn get_cell_id_from_role_name(&self, role_name: &RoleName) -> ConductorApiResult<CellId> {
        if is_clone_id(role_name) {
            let base_role_name = get_base_role_name_from_clone_id(role_name);

            let Some(role_cells) = self.app_info.cell_info.get(&base_role_name) else {
                return Err(ConductorApiError::CellNotFound);
            };

            let maybe_clone_cell: Option<ClonedCell> =
                role_cells.iter().find_map(|cell| match cell {
                    CellInfo::Cloned(cloned_cell) => {
                        if cloned_cell.clone_id.0.eq(role_name) {
                            Some(cloned_cell.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                });

            let clone_cell = maybe_clone_cell.ok_or(ConductorApiError::CellNotFound)?;
            Ok(clone_cell.cell_id)
        } else {
            let Some(role_cells) = self.app_info.cell_info.get(role_name) else {
                return Err(ConductorApiError::CellNotFound);
            };

            let maybe_provisioned: Option<ProvisionedCell> =
                role_cells.iter().find_map(|cell| match cell {
                    CellInfo::Provisioned(provisioned_cell) => Some(provisioned_cell.clone()),
                    _ => None,
                });

            let provisioned_cell = maybe_provisioned.ok_or(ConductorApiError::CellNotFound)?;
            Ok(provisioned_cell.cell_id)
        }
    }

    pub async fn dump_network_stats(&self) -> ConductorApiResult<kitsune2_api::TransportStats> {
        let msg = AppRequest::DumpNetworkStats;
        let response = self.inner.send(msg).await?;
        match response {
            AppResponse::NetworkStatsDumped(stats) => Ok(stats),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_network_metrics(
        &self,
        dna_hash: Option<holo_hash::DnaHash>,
        include_dht_summary: bool,
    ) -> ConductorApiResult<
        std::collections::HashMap<
            holo_hash::DnaHash,
            holochain_types::network::Kitsune2NetworkMetrics,
        >,
    > {
        let msg = AppRequest::DumpNetworkMetrics {
            dna_hash,
            include_dht_summary,
        };
        let response = self.inner.send(msg).await?;
        match response {
            AppResponse::NetworkMetricsDumped(metrics) => Ok(metrics),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn agent_info(
        &self,
        dna_hash: Option<holo_hash::DnaHash>,
    ) -> ConductorApiResult<Vec<String>> {
        let msg = AppRequest::AgentInfo { dna_hash };
        let response = self.inner.send(msg).await?;
        match response {
            AppResponse::AgentInfo(infos) => Ok(infos),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }
}

pub enum ZomeCallTarget {
    CellId(CellId),
    /// Call a cell by its role name.
    ///
    /// Note that when using clone cells, if you create them after creating the [AppWebsocket], you will need to call [AppWebsocket::refresh_app_info]
    /// for the right CellId to be found to make the call.
    RoleName(RoleName),
    /// Call a cell by its clone id.
    ///
    /// Note that when using clone cells, if you create them after creating the [AppWebsocket], you will need to call [AppWebsocket::refresh_app_info]
    /// for the right CellId to be found to make the call.
    CloneId(CloneId),
}

impl From<CellId> for ZomeCallTarget {
    fn from(cell_id: CellId) -> Self {
        ZomeCallTarget::CellId(cell_id)
    }
}

impl From<RoleName> for ZomeCallTarget {
    fn from(role_name: RoleName) -> Self {
        ZomeCallTarget::RoleName(role_name)
    }
}

impl From<CloneId> for ZomeCallTarget {
    fn from(clone_id: CloneId) -> Self {
        ZomeCallTarget::CloneId(clone_id)
    }
}

fn is_clone_id(role_name: &RoleName) -> bool {
    role_name.as_str().contains('.')
}

fn get_base_role_name_from_clone_id(role_name: &RoleName) -> RoleName {
    RoleName::from(
        role_name
            .as_str()
            .split('.')
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .first()
            .unwrap(),
    )
}
