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
            let (provenance, cap_grant) = {
                match call_context.auth() {
                    Some((provenance, cap_secret)) => {
                        let cap_grant = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .valid_cap_grant(
                                &(call_context.zome.zome_name().clone(), call_context.function_name().clone()),
                                &provenance,
                                Some(&cap_secret),
                            ).map_err(|e| WasmError::Host(e.to_string()))?
                            // This is really a problem.
                            // It means that the host function calling into `call_info`
                            // is using a cap secret that never had authorization to call in the first place.
                            // The host must NEVER allow this so `None` is a critical bug.
                            .unwrap();
                        (provenance, cap_grant)
                    },
                    None => {
                        let author = call_context.host_context.workspace().source_chain().agent_pubkey().clone();
                        (author.clone(), CapGrant::ChainAuthor(author))
                    }
                }
            };
            Ok(CallInfo {
                as_at: call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .persisted_chain_head(),
                provenance,
                cap_grant,
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
