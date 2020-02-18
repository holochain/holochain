use crate::api::{ConductorCellApi, ConductorExternalApi};
use async_trait::async_trait;

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ConductorExternalApi<ConductorCellApi>);
}
