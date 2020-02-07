use crate::api::ConductorApiExternal;
use async_trait::async_trait;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ConductorApiExternal);
}
