use crate::conductor::{
    api::{AdminMethod, ConductorRequest, ExternalConductorApi},
    interface::interface::Interface,
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::*;

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorApi
pub struct ChannelInterface {
    rx: mpsc::Receiver<bool>,
}

impl ChannelInterface {
    pub fn new(rx: mpsc::Receiver<bool>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl Interface for ChannelInterface {
    async fn spawn(mut self, api: ExternalConductorApi) {
        debug!("spawn start");
        while let Some(true) = self.rx.recv().await {
            let _ = api
                .handle_request(ConductorRequest::Admin {
                    request: Box::new(AdminMethod::Start("cell-handle".into())),
                })
                .await;
        }
    }
}
