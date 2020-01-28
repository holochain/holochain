use crate::api::ConductorApiExternal;
use async_trait::async_trait;
use sx_core::cell::CellApi;

#[async_trait]
pub trait Interface<Cell: CellApi, Api: ConductorApiExternal<Cell>> {
    async fn spawn(self, api: Api)
    where
        Api: 'async_trait,
        Cell: 'async_trait;
}
