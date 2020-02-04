use crate::{
    api::{self, ConductorApiExternal},
    interface::interface::Interface,
};
use async_trait::async_trait;
use futures::{channel::mpsc, stream::StreamExt};
use sx_core::cell::CellApi;

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ConductorApiExternal
pub struct PuppetInterface {
    rx: mpsc::UnboundedReceiver<bool>,
}

impl PuppetInterface {
    pub fn new(rx: mpsc::UnboundedReceiver<bool>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl<Cell: CellApi, Api: ConductorApiExternal<Cell>> Interface<Cell, Api> for PuppetInterface {
    async fn spawn(mut self, mut api: Api)
    where
        Api: 'async_trait,
        Cell: 'async_trait,
    {
        dbg!("spawn start");
        while let Some(true) = self.rx.next().await {
            dbg!("x");
            api.admin(api::AdminMethod::Start("cell-handle".into()));
        }
    }
}
