use anyhow::{anyhow, Context};
use holo_hash::{AgentPubKey, DnaHash};
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, AppAuthenticationToken, AppInfo,
    AppInterfaceInfo, AppRequest, AppResponse, CellInfo, NetworkInfo,
};
use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use std::sync::Arc;

pub struct AppClient {
    tx: WebsocketSender,
    rx: tokio::task::JoinHandle<()>,
}

impl Drop for AppClient {
    fn drop(&mut self) {
        self.rx.abort();
    }
}

impl AppClient {
    /// Creates a App websocket client which can send messages but ignores any incoming messages
    async fn connect(
        addr: std::net::SocketAddr,
        token: AppAuthenticationToken,
    ) -> anyhow::Result<Self> {
        let (tx, mut rx) = connect(
            Arc::new(WebsocketConfig::CLIENT_DEFAULT),
            ConnectRequest::new(addr).try_set_header("origin", HC_TERM_ORIGIN)?,
        )
        .await?;

        let rx = tokio::task::spawn(async move { while rx.recv::<AppResponse>().await.is_ok() {} });

        tx.authenticate(AppAuthenticationRequest { token })
            .await
            .context("Failed to authenticate app client")?;

        Ok(AppClient { tx, rx })
    }

    pub async fn discover_network_info_params(
        &mut self,
        app_id: InstalledAppId,
    ) -> anyhow::Result<(AgentPubKey, Vec<(String, DnaHash)>)> {
        let app_info = self
            .app_info()
            .await?
            .ok_or(anyhow!("App not found {}", app_id))?;

        let agent = app_info.agent_pub_key;
        let named_dna_hashes: Vec<(String, DnaHash)> = app_info
            .cell_info
            .values()
            .flat_map(|c| {
                c.iter().filter_map(|c| match c {
                    CellInfo::Provisioned(p) => {
                        Some((p.name.clone(), p.cell_id.dna_hash().clone()))
                    }
                    _ => None,
                })
            })
            .collect();

        Ok((agent, named_dna_hashes))
    }

    pub async fn network_info(
        &mut self,
        agent: AgentPubKey,
        dna_hashes: Vec<DnaHash>,
    ) -> anyhow::Result<Vec<NetworkInfo>> {
        let r = NetworkInfoRequestPayload {
            agent_pub_key: agent,
            dnas: dna_hashes,
            last_time_queried: None,
        };
        let msg = AppRequest::NetworkInfo(Box::new(r));
        let response = self.send(msg).await?;
        match response {
            AppResponse::NetworkInfo(infos) => Ok(infos),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn app_info(&mut self) -> anyhow::Result<Option<AppInfo>> {
        let msg = AppRequest::AppInfo;
        let response = self.send(msg).await?;
        match response {
            AppResponse::AppInfo(app_info) => Ok(app_info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn send(&mut self, msg: AppRequest) -> anyhow::Result<AppResponse> {
        let response = self.tx.request(msg).await?;

        match response {
            AppResponse::Error(error) => Err(anyhow!("External error: {:?}", error)),
            _ => Ok(response),
        }
    }
}

pub struct AdminClient {
    tx: WebsocketSender,
    rx: tokio::task::JoinHandle<()>,
    addr: std::net::SocketAddr,
}

impl Drop for AdminClient {
    fn drop(&mut self) {
        self.rx.abort();
    }
}

const HC_TERM_ORIGIN: &str = "hcterm";

impl AdminClient {
    /// Creates an Admin websocket client which can send messages but ignores any incoming messages
    pub async fn connect(addr: std::net::SocketAddr) -> anyhow::Result<Self> {
        let (tx, mut rx) = connect(Arc::new(WebsocketConfig::CLIENT_DEFAULT), addr).await?;

        let rx =
            tokio::task::spawn(async move { while rx.recv::<AdminResponse>().await.is_ok() {} });

        Ok(AdminClient { tx, rx, addr })
    }

    pub async fn connect_app_client(
        &mut self,
        installed_app_id: InstalledAppId,
    ) -> anyhow::Result<AppClient> {
        let app_interfaces = self.list_app_interfaces().await?;

        let app_port = if let Some(interface) =
            Self::select_usable_app_interface(app_interfaces, installed_app_id.clone())
        {
            interface.port
        } else {
            self.attach_app_interface(0).await?
        };

        let app_addr = (self.addr.ip(), app_port).into();

        let issue_token_response = self
            .tx
            .request(AdminRequest::IssueAppAuthenticationToken(
                installed_app_id.into(),
            ))
            .await?;
        let token = match issue_token_response {
            AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
            _ => anyhow::bail!("Unexpected response {:?}", issue_token_response),
        };

        AppClient::connect(app_addr, token).await
    }

    async fn list_app_interfaces(&mut self) -> anyhow::Result<Vec<AppInterfaceInfo>> {
        let msg = AdminRequest::ListAppInterfaces;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfacesListed(interfaces) => Ok(interfaces),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn attach_app_interface(&mut self, port: u16) -> anyhow::Result<u16> {
        let msg = AdminRequest::AttachAppInterface {
            port: Some(port),
            allowed_origins: HC_TERM_ORIGIN.to_string().into(),
            installed_app_id: None,
        };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfaceAttached { port } => Ok(port),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    fn select_usable_app_interface(
        interfaces: impl IntoIterator<Item = AppInterfaceInfo>,
        installed_app_id: InstalledAppId,
    ) -> Option<AppInterfaceInfo> {
        interfaces.into_iter().find(|interface| {
            let can_use_app_id = interface.installed_app_id.is_none()
                || interface.installed_app_id.clone().unwrap() == installed_app_id;

            let can_use_origin = match interface.allowed_origins {
                AllowedOrigins::Any => true,
                AllowedOrigins::Origins(ref origins) => origins.contains(HC_TERM_ORIGIN),
            };

            can_use_app_id && can_use_origin
        })
    }

    async fn send(&mut self, msg: AdminRequest) -> anyhow::Result<AdminResponse> {
        let response = self.tx.request(msg).await?;

        match response {
            AdminResponse::Error(error) => Err(anyhow!("External error: {:?}", error)),
            _ => Ok(response),
        }
    }
}
