use crate::error::{ConductorApiError, ConductorApiResult};
use crate::util::AbortOnDropHandle;
use holo_hash::DnaHash;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationToken, AppAuthenticationTokenIssued, AppInfo,
    AppInterfaceInfo, AppStatusFilter, FullStateDump, IssueAppAuthenticationTokenPayload,
    RevokeAgentKeyPayload, StorageInfo,
};
use holochain_types::websocket::AllowedOrigins;
use holochain_types::{
    dna::AgentPubKey,
    prelude::{CellId, DeleteCloneCellPayload, InstallAppPayload, UpdateCoordinatorsPayload},
};
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use holochain_zome_types::{
    capability::GrantedFunctions,
    prelude::{DnaDef, GrantZomeCallCapabilityPayload, Record},
};
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use std::{net::ToSocketAddrs, sync::Arc};

/// A websocket connection to the Holochain Conductor admin interface.
#[derive(Clone)]
pub struct AdminWebsocket {
    tx: WebsocketSender,
    _poll_handle: Arc<AbortOnDropHandle>,
}

impl std::fmt::Debug for AdminWebsocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminWebsocket").finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnableAppResponse {
    pub app: AppInfo,
    pub errors: Vec<(CellId, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizeSigningCredentialsPayload {
    pub cell_id: CellId,
    pub functions: Option<GrantedFunctions>,
}

impl AdminWebsocket {
    /// Connect to a Conductor API admin websocket.
    ///
    /// `socket_addr` is a websocket address that implements [ToSocketAddr](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html#tymethod.to_socket_addrs).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[tokio::main]
    /// # async fn main() {
    /// use std::net::Ipv4Addr;
    /// use holochain_client::AdminWebsocket;
    ///
    /// let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, 30_000)).await.unwrap();
    /// # }
    /// ```
    ///
    /// As string: `"localhost:30000"`
    ///
    /// As tuple: `([127.0.0.1], 30000)`
    pub async fn connect(socket_addr: impl ToSocketAddrs) -> ConductorApiResult<Self> {
        Self::connect_with_config(socket_addr, Arc::new(WebsocketConfig::CLIENT_DEFAULT)).await
    }

    /// Connect to a Conductor API admin websocket with a custom [WebsocketConfig].
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
    /// use holochain_client::{AdminWebsocket, AllowedOrigins, WebsocketConfig};
    ///
    /// // Create a client config from the default and set a timeout that is lower than the default
    /// let mut client_config = WebsocketConfig::CLIENT_DEFAULT;
    /// client_config.default_request_timeout = Duration::from_secs(10);
    ///
    /// let client_config = Arc::new(client_config);
    ///
    /// let admin_ws = AdminWebsocket::connect_with_config((Ipv4Addr::LOCALHOST, 30_000), client_config).await.unwrap();
    /// # }
    /// ```
    pub async fn connect_with_config(
        socket_addr: impl ToSocketAddrs,
        websocket_config: Arc<WebsocketConfig>,
    ) -> ConductorApiResult<Self> {
        let mut last_err = None;
        for addr in socket_addr.to_socket_addrs()? {
            let request: ConnectRequest = addr.into();

            match Self::connect_with_request_and_config(request, websocket_config.clone()).await {
                Ok(admin_ws) => return Ok(admin_ws),
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

    /// Connect to a Conductor API admin websocket with a custom [ConnectRequest] and [WebsocketConfig].
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
    /// use holochain_client::{AdminWebsocket, AllowedOrigins, WebsocketConfig, ConnectRequest};
    ///
    /// // Use the default client config
    /// let mut client_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);
    ///
    /// // Attempt to connect to Holochain on one of these interfaces on port 30,000
    /// let connect_to = vec![
    ///     SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 30_000),
    ///     SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 30_000),
    /// ];
    /// for addr in connect_to {
    ///     // Send a request with a custom origin header to identify the client
    ///     let mut request: ConnectRequest = addr.into();
    ///     let request = request
    ///         .try_set_header("Origin", "my_cli_app")
    ///         .unwrap();
    ///
    ///     match AdminWebsocket::connect_with_request_and_config(request, client_config.clone()).await {
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
    ) -> ConductorApiResult<Self> {
        let (tx, mut rx) = connect(websocket_config.clone(), request).await?;

        // WebsocketReceiver needs to be polled in order to receive responses
        // from remote to sender requests.
        let poll_handle =
            tokio::task::spawn(async move { while rx.recv::<AdminResponse>().await.is_ok() {} });

        Ok(Self {
            tx,
            _poll_handle: Arc::new(AbortOnDropHandle::new(poll_handle.abort_handle())),
        })
    }

    /// Issue an app authentication token for the specified app.
    ///
    /// A token is required to create an [AppWebsocket](crate::AppWebsocket) connection.
    pub async fn issue_app_auth_token(
        &self,
        payload: IssueAppAuthenticationTokenPayload,
    ) -> ConductorApiResult<AppAuthenticationTokenIssued> {
        let response = self
            .send(AdminRequest::IssueAppAuthenticationToken(payload))
            .await?;
        match response {
            AdminResponse::AppAuthenticationTokenIssued(issued) => Ok(issued),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn revoke_app_authentication_token(
        &self,
        token: AppAuthenticationToken,
    ) -> ConductorApiResult<()> {
        let response = self
            .send(AdminRequest::RevokeAppAuthenticationToken(token))
            .await?;
        match response {
            AdminResponse::AppAuthenticationTokenRevoked => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn generate_agent_pub_key(&self) -> ConductorApiResult<AgentPubKey> {
        // Create agent key in Lair and save it in file
        let response = self.send(AdminRequest::GenerateAgentPubKey).await?;
        match response {
            AdminResponse::AgentPubKeyGenerated(key) => Ok(key),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn revoke_agent_key(
        &self,
        app_id: String,
        agent_key: AgentPubKey,
    ) -> ConductorApiResult<Vec<(CellId, String)>> {
        let response = self
            .send(AdminRequest::RevokeAgentKey(Box::new(
                RevokeAgentKeyPayload { app_id, agent_key },
            )))
            .await?;
        match response {
            AdminResponse::AgentKeyRevoked(errors) => Ok(errors),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    /// List all app interfaces attached to the conductor.
    ///
    /// See the documentation for [AdminWebsocket::attach_app_interface] to understand the content
    /// of `AppInterfaceInfo` and help you to select an appropriate interface to connect to.
    pub async fn list_app_interfaces(&self) -> ConductorApiResult<Vec<AppInterfaceInfo>> {
        let msg = AdminRequest::ListAppInterfaces;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfacesListed(interfaces) => Ok(interfaces),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    /// Attach an app interface to the conductor.
    ///
    /// This will create a new websocket on the specified port. Alternatively, specify the port as
    /// 0 to allow the OS to choose a port. The selected port will be returned so you know where
    /// to connect your app client.
    ///
    /// Allowed origins can be used to restrict which domains can connect to the interface.
    /// This is used to protect the interface from scripts running in web pages. In development it
    /// is acceptable to use `AllowedOrigins::All` to allow all connections. In production you
    /// should consider setting an explicit list of origins, such as `"my_cli_app".to_string().into()`.
    ///
    /// If you want to restrict this app interface so that it is only accessible to a specific
    /// installed app then you can provide the installed_app_id. The client will still need to
    /// authenticate with a valid token for the same app, but clients for other apps will not be
    /// able to connect. If you want to allow all apps to connect then set this to `None`.
    pub async fn attach_app_interface(
        &self,
        port: u16,
        allowed_origins: AllowedOrigins,
        installed_app_id: Option<String>,
    ) -> ConductorApiResult<u16> {
        let msg = AdminRequest::AttachAppInterface {
            port: Some(port),
            allowed_origins,
            installed_app_id,
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfaceAttached { port } => Ok(port),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorApiResult<Vec<AppInfo>> {
        let response = self.send(AdminRequest::ListApps { status_filter }).await?;
        match response {
            AdminResponse::AppsListed(apps_infos) => Ok(apps_infos),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn install_app(&self, payload: InstallAppPayload) -> ConductorApiResult<AppInfo> {
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let response = self.send(msg).await?;

        match response {
            AdminResponse::AppInstalled(app_info) => Ok(app_info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn uninstall_app(
        &self,
        installed_app_id: String,
        force: bool,
    ) -> ConductorApiResult<()> {
        let msg = AdminRequest::UninstallApp {
            installed_app_id,
            force,
        };
        let response = self.send(msg).await?;

        match response {
            AdminResponse::AppUninstalled => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn list_dnas(&self) -> ConductorApiResult<Vec<DnaHash>> {
        let response = self.send(AdminRequest::ListDnas).await?;
        match response {
            AdminResponse::DnasListed(dnas) => Ok(dnas),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn enable_app(
        &self,
        installed_app_id: String,
    ) -> ConductorApiResult<EnableAppResponse> {
        let msg = AdminRequest::EnableApp { installed_app_id };
        let response = self.send(msg).await?;

        match response {
            AdminResponse::AppEnabled { app, errors } => Ok(EnableAppResponse { app, errors }),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn disable_app(&self, installed_app_id: String) -> ConductorApiResult<()> {
        let msg = AdminRequest::DisableApp { installed_app_id };
        let response = self.send(msg).await?;

        match response {
            AdminResponse::AppDisabled => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn list_cell_ids(&self) -> ConductorApiResult<Vec<CellId>> {
        let response = self.send(AdminRequest::ListCellIds).await?;
        match response {
            AdminResponse::CellIdsListed(cell_ids) => Ok(cell_ids),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn get_dna_definition(&self, hash: DnaHash) -> ConductorApiResult<DnaDef> {
        let msg = AdminRequest::GetDnaDefinition(Box::new(hash));
        let response = self.send(msg).await?;
        match response {
            AdminResponse::DnaDefinitionReturned(dna_definition) => Ok(dna_definition),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn grant_zome_call_capability(
        &self,
        payload: GrantZomeCallCapabilityPayload,
    ) -> ConductorApiResult<()> {
        let msg = AdminRequest::GrantZomeCallCapability(Box::new(payload));
        let response = self.send(msg).await?;

        match response {
            AdminResponse::ZomeCallCapabilityGranted => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn delete_clone_cell(
        &self,
        payload: DeleteCloneCellPayload,
    ) -> ConductorApiResult<()> {
        let msg = AdminRequest::DeleteCloneCell(Box::new(payload));
        let response = self.send(msg).await?;
        match response {
            AdminResponse::CloneCellDeleted => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn storage_info(&self) -> ConductorApiResult<StorageInfo> {
        let msg = AdminRequest::StorageInfo;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::StorageInfo(info) => Ok(info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_network_stats(&self) -> ConductorApiResult<kitsune2_api::TransportStats> {
        let msg = AdminRequest::DumpNetworkStats;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::NetworkStatsDumped(stats) => Ok(stats),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_state(&self, cell_id: CellId) -> ConductorApiResult<String> {
        let msg = AdminRequest::DumpState {
            cell_id: Box::new(cell_id),
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::StateDumped(state) => Ok(state),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_conductor_state(&self) -> ConductorApiResult<String> {
        let msg = AdminRequest::DumpConductorState;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::ConductorStateDumped(state) => Ok(state),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_full_state(
        &self,
        cell_id: CellId,
        dht_ops_cursor: Option<u64>,
    ) -> ConductorApiResult<FullStateDump> {
        let msg = AdminRequest::DumpFullState {
            cell_id: Box::new(cell_id),
            dht_ops_cursor,
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::FullStateDumped(state) => Ok(state),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn dump_network_metrics(
        &self,
        dna_hash: Option<DnaHash>,
        include_dht_summary: bool,
    ) -> ConductorApiResult<
        std::collections::HashMap<DnaHash, holochain_types::network::Kitsune2NetworkMetrics>,
    > {
        let msg = AdminRequest::DumpNetworkMetrics {
            dna_hash,
            include_dht_summary,
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::NetworkMetricsDumped(metrics) => Ok(metrics),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn update_coordinators(
        &self,
        update_coordinators_payload: UpdateCoordinatorsPayload,
    ) -> ConductorApiResult<()> {
        let msg = AdminRequest::UpdateCoordinators(Box::new(update_coordinators_payload));
        let response = self.send(msg).await?;
        match response {
            AdminResponse::CoordinatorsUpdated => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn graft_records(
        &self,
        cell_id: CellId,
        validate: bool,
        records: Vec<Record>,
    ) -> ConductorApiResult<()> {
        let msg = AdminRequest::GraftRecords {
            cell_id,
            validate,
            records,
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::RecordsGrafted => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn agent_info(&self, cell_id: Option<CellId>) -> ConductorApiResult<Vec<String>> {
        let msg = AdminRequest::AgentInfo { cell_id };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AgentInfo(agent_info) => Ok(agent_info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn add_agent_info(&self, agent_infos: Vec<String>) -> ConductorApiResult<()> {
        let msg = AdminRequest::AddAgentInfo { agent_infos };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AgentInfoAdded => Ok(()),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn authorize_signing_credentials(
        &self,
        request: AuthorizeSigningCredentialsPayload,
    ) -> ConductorApiResult<crate::signing::client_signing::SigningCredentials> {
        use holochain_zome_types::capability::{ZomeCallCapGrant, CAP_SECRET_BYTES};
        use rand::{rngs::OsRng, RngCore};
        use std::collections::BTreeSet;

        let mut csprng = OsRng;
        let keypair = ed25519_dalek::SigningKey::generate(&mut csprng);
        let public_key = keypair.verifying_key();
        let signing_agent_key = AgentPubKey::from_raw_32(public_key.as_bytes().to_vec());

        let mut cap_secret = [0; CAP_SECRET_BYTES];
        csprng.fill_bytes(&mut cap_secret);

        self.grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id: request.cell_id,
            cap_grant: ZomeCallCapGrant {
                tag: "zome-call-signing-key".to_string(),
                access: holochain_zome_types::capability::CapAccess::Assigned {
                    secret: cap_secret.into(),
                    assignees: BTreeSet::from([signing_agent_key.clone()]),
                },
                functions: request.functions.unwrap_or(GrantedFunctions::All),
            },
        })
        .await?;

        Ok(crate::signing::client_signing::SigningCredentials {
            signing_agent_key,
            keypair,
            cap_secret: cap_secret.into(),
        })
    }

    async fn send(&self, msg: AdminRequest) -> ConductorApiResult<AdminResponse> {
        let response: AdminResponse = self
            .tx
            .request(msg)
            .await
            .map_err(ConductorApiError::WebsocketError)?;
        match response {
            AdminResponse::Error(error) => Err(ConductorApiError::ExternalApiWireError(error)),
            _ => Ok(response),
        }
    }
}
