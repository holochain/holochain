use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::EntryDefsHostAccess;
use crate::core::ribosome::EntryDefsInvocation;
use crate::core::ribosome::EntryDefsResult;

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
                entry_defs: {
                    match ribosome.run_entry_defs(EntryDefsHostAccess, EntryDefsInvocation).map_err(|e| WasmError::Host(e.to_string()))? {
                        EntryDefsResult::Err(zome, error_string) => return Err(WasmError::Host(format!("{}: {}", zome, error_string))),
                        EntryDefsResult::Defs(defs) => {
                            match defs.get(call_context.zome.zome_name()) {
                                Some(entry_defs) => entry_defs.clone(),
                                None => Vec::new().into(),
                            }
                        },
                    }
                },
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

    #[tokio::test(flavor = "multi_thread")]
    async fn zome_info_entry_defs() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let zome_info: ZomeInfo = crate::call_test_ribosome!(host_access, TestWasm::EntryDefs, "zome_info", ()).unwrap();
        assert_eq!(
            zome_info.entry_defs,
            vec![
                EntryDef {
                    id: "post".into(),
                    visibility: Default::default(),
                    crdt_type: Default::default(),
                    required_validations: Default::default(),
                    required_validation_type: Default::default(),
                },
                EntryDef {
                    id: "comment".into(),
                    visibility: EntryVisibility::Private,
                    crdt_type: Default::default(),
                    required_validations: Default::default(),
                    required_validation_type: Default::default(),
                }
            ].into(),
        );
    }
}
