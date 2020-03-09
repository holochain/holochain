use crate::{CellConductorApiT, ConductorApiResult};
use async_trait::async_trait;
use sx_types::nucleus::{ZomeInvocation, ZomeInvocationResponse};


/// The methods that a concrete Cell type must implement in order to work with
/// [CellConductorApiT]
#[async_trait]
pub trait ApiCellT: Sized + Send + Sync {
    type Api: CellConductorApiT;

    async fn invoke_zome(
        &self,
        conductor_api: Self::Api,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}
