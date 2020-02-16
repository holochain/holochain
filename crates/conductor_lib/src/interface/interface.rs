use crate::api::ConductorExternalApi;
use async_trait::async_trait;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ConductorExternalApi);
}
