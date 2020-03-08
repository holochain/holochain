use crate::ConductorApiResult;
use sx_types::nucleus::ZomeInvocationResponse;
use sx_types::nucleus::ZomeInvocation;
use crate::CellConductorInterfaceT;
use async_trait::async_trait;

#[async_trait]
pub trait CellT: Sized {
    type Interface: CellConductorInterfaceT;

    async fn invoke_zome(
        &self,
        conductor_api: Self::Interface,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}

