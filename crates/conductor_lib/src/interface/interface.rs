use crate::api::ConductorApiExternal;
use skunkworx_core::cell::CellApi;
use async_trait::async_trait;

#[async_trait]
pub trait Interface<Cell: CellApi, Api: ConductorApiExternal<Cell>> {
    async fn spawn(self, api: Api)
    where
        Api: 'async_trait,
        Cell: 'async_trait;
}
