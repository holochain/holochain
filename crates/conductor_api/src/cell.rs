use crate::ConductorApiResult;
use sx_types::nucleus::ZomeInvocationResponse;
use sx_types::nucleus::ZomeInvocation;
use crate::CellConductorApiT;
use async_trait::async_trait;

#[async_trait]
pub trait CellT: Sized + Send + Sync {
    type Api: CellConductorApiT;

    async fn invoke_zome(
        &self,
        conductor_api: Self::Api,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}

