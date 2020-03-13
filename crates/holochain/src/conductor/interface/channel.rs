use crate::conductor::{
    api::{AdminMethod, ExternalConductorApi},
    interface::interface::Interface,
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::*;

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorApi
pub struct ChannelInterface {
    rx: mpsc::UnboundedReceiver<bool>,
}

impl ChannelInterface {
    pub fn new(rx: mpsc::UnboundedReceiver<bool>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl Interface for ChannelInterface {
    #[instrument(skip(self, api))]
    async fn spawn(mut self, mut api: ExternalConductorApi) {
        debug!("spawn start");
        while let Some(true) = self.rx.recv().await {
            if let Err(err) = api.admin(AdminMethod::Start("cell-handle".into())).await {
                error!("Error calling admin interface function: {}", err);
            };
        }
    }
}
