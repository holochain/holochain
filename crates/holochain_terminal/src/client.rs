use anyhow::anyhow;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_client::{AdminWebsocket, AppWebsocket, ClientAgentSigner, DynAgentSigner};
use holochain_conductor_api::{AppAuthenticationToken, AppInterfaceInfo, CellInfo};
use holochain_types::network::Kitsune2NetworkMetrics;
use holochain_types::prelude::InstalledAppId;
use holochain_types::websocket::AllowedOrigins;
use std::collections::HashMap;

const HC_TERM_ORIGIN: &str = "hcterm";

pub struct AppClient {
    client: AppWebsocket,
}

impl AppClient {
    /// Creates a App websocket client which can send messages but ignores any incoming messages
    async fn connect(
        addr: std::net::SocketAddr,
        token: AppAuthenticationToken,
    ) -> anyhow::Result<Self> {
        let client = AppWebsocket::connect(
            addr,
            token,
            DynAgentSigner::from(ClientAgentSigner::new()),
            None,
        )
        .await?;
        Ok(AppClient { client })
    }

    pub async fn discover_network_metrics_params(
        &mut self,
        app_id: InstalledAppId,
    ) -> anyhow::Result<(AgentPubKey, Vec<(String, DnaHash)>)> {
        let app_info = self
            .client
            .app_info()
            .await?
            .ok_or(anyhow!("App not found {}", app_id))?;

        let agent = app_info.agent_pub_key;
        let named_dna_hashes: Vec<(String, DnaHash)> = app_info
            .cell_info
            .values()
            .flat_map(|c| {
                c.iter().filter_map(|c| match c {
                    CellInfo::Provisioned(p) => Some((p.name.clone(), p.dna_id.dna_hash().clone())),
                    _ => None,
                })
            })
            .collect();

        Ok((agent, named_dna_hashes))
    }

    pub async fn network_metrics(
        &mut self,
    ) -> anyhow::Result<HashMap<DnaHash, Kitsune2NetworkMetrics>> {
        Ok(self.client.dump_network_metrics(None, false).await?)
    }
}

pub struct AdminClient {
    client: AdminWebsocket,
    addr: std::net::SocketAddr,
}

impl AdminClient {
    /// Creates an Admin websocket client which can send messages but ignores any incoming messages
    pub async fn connect(addr: std::net::SocketAddr) -> anyhow::Result<Self> {
        let client = AdminWebsocket::connect(addr, None).await?;
        Ok(AdminClient { client, addr })
    }

    pub async fn connect_app_client(
        &mut self,
        installed_app_id: InstalledAppId,
    ) -> anyhow::Result<AppClient> {
        let app_interfaces = self.client.list_app_interfaces().await?;

        let app_port = if let Some(interface) =
            Self::select_usable_app_interface(app_interfaces, installed_app_id.clone())
        {
            interface.port
        } else {
            self.client
                .attach_app_interface(0, HC_TERM_ORIGIN.to_string().into(), None)
                .await?
        };

        let app_addr = (self.addr.ip(), app_port).into();

        let token = self
            .client
            .issue_app_auth_token(installed_app_id.into())
            .await?;

        AppClient::connect(app_addr, token.token).await
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
}
