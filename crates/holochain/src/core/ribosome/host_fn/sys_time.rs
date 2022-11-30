use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::access::Permission;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::Timestamp;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

pub fn sys_time(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<Timestamp, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } => Ok(holochain_zome_types::Timestamp::now()),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "sys_time".into(),
            )
            .to_string(),
        )).into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::Timestamp;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_sys_time_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::SysTime).await;
        let _: Timestamp = conductor.call(&alice, "sys_time", ()).await;
    }
}
