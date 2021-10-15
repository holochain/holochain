use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::CallInfo;
use holochain_types::prelude::*;

pub fn call_info(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<CallInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings: Permission::Allow, .. } => {
            Ok(CallInfo {
                as_at: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .persisted_chain_head()
            })
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_call_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let call_info: CallInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "call_info", ()).unwrap();
        assert_eq!(call_info.as_at.1, 2);
    }
}
