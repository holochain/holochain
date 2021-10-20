use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

pub fn zome_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            Ok(ZomeInfo {
                name: call_context.zome.zome_name().clone(),
                id: ribosome
                    .zome_to_id(&call_context.zome)
                    .expect("Failed to get ID for current zome"),
                // @TODO
                // public_token: "".into(),
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
    async fn invoke_import_zome_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let zome_info: ZomeInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "zome_info", ()).unwrap();
        assert_eq!(zome_info.name, "zome_info".into());
    }
}
