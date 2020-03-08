use async_trait::async_trait;
use crate::api::ExternalConductorInterface;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ExternalConductorInterface);
}
