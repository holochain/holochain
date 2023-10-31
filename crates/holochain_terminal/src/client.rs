use anyhow::anyhow;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppInfo, AppRequest, AppResponse, CellInfo, NetworkInfo,
};
use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_websocket::{
    connect, WebsocketConfig, WebsocketError, WebsocketReceiver, WebsocketSender,
};
use std::sync::Arc;
use url::Url;

pub struct AppClient {
    tx: WebsocketSender,
    rx: WebsocketReceiver,
}

impl AppClient {
    async fn connect(url: &Url) -> anyhow::Result<Self> {
        let (tx, rx) = connect(url.clone().into(), Arc::new(WebsocketConfig::default())).await?;

        Ok(AppClient { tx, rx })
    }

    pub async fn discover_network_info_params(
        &mut self,
        app_id: InstalledAppId,
    ) -> anyhow::Result<(AgentPubKey, Vec<(String, DnaHash)>)> {
        let app_info = self
            .app_info(app_id.clone())
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

    async fn app_info(&mut self, app_id: InstalledAppId) -> anyhow::Result<Option<AppInfo>> {
        let msg = AppRequest::AppInfo {
            installed_app_id: app_id,
        };
        let response = self.send(msg).await?;
        match response {
            AppResponse::AppInfo(app_info) => Ok(app_info),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn send(&mut self, msg: AppRequest) -> anyhow::Result<AppResponse> {
        let response = self.tx.request(msg).await.map_err(WebsocketError::from)?;

        match response {
            AppResponse::Error(error) => Err(anyhow!("External error: {:?}", error)),
            _ => Ok(response),
        }
    }
}

impl Drop for AppClient {
    fn drop(&mut self) {
        if let Some(h) = self.rx.take_handle() {
            h.close();
        }
    }
}

pub struct AdminClient {
    tx: WebsocketSender,
    rx: WebsocketReceiver,
    url: Url,
}

impl AdminClient {
    pub async fn connect(url: &Url) -> anyhow::Result<Self> {
        let (tx, rx) = connect(url.clone().into(), Arc::new(WebsocketConfig::default())).await?;

        Ok(AdminClient {
            tx,
            rx,
            url: url.clone(),
        })
    }

    pub async fn connect_app_client(&mut self) -> anyhow::Result<AppClient> {
        let app_interfaces = self.list_app_interfaces().await?;
        let app_port = if app_interfaces.is_empty() {
            self.attach_app_interface(0).await?
        } else {
            *app_interfaces.first().unwrap()
        };

        let mut app_url = self.url.clone();
        app_url
            .set_port(Some(app_port))
            .map_err(|_| anyhow!("Failed to set port on app_url"))?;

        AppClient::connect(&app_url).await
    }

    async fn list_app_interfaces(&mut self) -> anyhow::Result<Vec<u16>> {
        let msg = AdminRequest::ListAppInterfaces;
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfacesListed(ports) => Ok(ports),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn attach_app_interface(&mut self, port: u16) -> anyhow::Result<u16> {
        let msg = AdminRequest::AttachAppInterface { port: Some(port) };
        let response = self.send(msg).await?;
        match response {
            AdminResponse::AppInterfaceAttached { port } => Ok(port),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    async fn send(&mut self, msg: AdminRequest) -> anyhow::Result<AdminResponse> {
        let response = self.tx.request(msg).await.map_err(WebsocketError::from)?;

        match response {
            AdminResponse::Error(error) => Err(anyhow!("External error: {:?}", error)),
            _ => Ok(response),
        }
    }
}

impl Drop for AdminClient {
    fn drop(&mut self) {
        if let Some(h) = self.rx.take_handle() {
            h.close();
        }
    }
}
