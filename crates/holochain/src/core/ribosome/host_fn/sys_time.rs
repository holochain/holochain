use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::access::Permission;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::Timestamp;
use std::sync::Arc;

pub fn sys_time(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<Timestamp, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } => Ok(holochain_types::timestamp::now()),
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_sys_time_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let _: core::time::Duration =
            crate::call_test_ribosome!(host_access, TestWasm::SysTime, "sys_time", ()).unwrap();
    }
}
