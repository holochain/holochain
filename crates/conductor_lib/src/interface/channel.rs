use crate::{
    api::{
        ExternalConductorInterface, {self},
    },
    interface::interface::Interface,
};
use api::CellConductorInterface;
use async_trait::async_trait;
use tokio::sync::mpsc;
use log::*;
use sx_conductor_api::{AdminMethod, ExternalConductorInterfaceT};

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorInterface
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
    async fn spawn(mut self, mut api: ExternalConductorInterface<CellConductorInterface>) {
        dbg!("spawn start");
        while let Some(true) = self.rx.recv().await {
            if let Err(err) = api.admin(AdminMethod::Start("cell-handle".into())).await {
                error!("Error calling admin interface function: {}", err);
            };
        }
    }
}
