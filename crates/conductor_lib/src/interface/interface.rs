use async_trait::async_trait;
use super::ExternalConductorInterface;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ExternalConductorInterface);
}
