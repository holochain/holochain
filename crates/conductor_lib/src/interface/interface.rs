use async_trait::async_trait;
use crate::api::ExternalConductorApi;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ExternalConductorApi);
}
