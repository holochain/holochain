use crate::conductor::{api::ExternalConductorApi, interface::interface::Interface};
use async_trait::async_trait;
use log::*;
use sx_conductor_api::external::AdminMethod;
use tokio::sync::mpsc;

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
    async fn spawn(mut self, mut api: ExternalConductorApi) {
        dbg!("spawn start");
        while let Some(true) = self.rx.recv().await {
            if let Err(err) = api.admin(AdminMethod::Start("cell-handle".into())).await {
                error!("Error calling admin interface function: {}", err);
            };
        }
    }
}
