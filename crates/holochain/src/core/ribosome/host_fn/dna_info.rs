use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::DnaInfo;
use crate::core::ribosome::HostFnAccess;
use holo_hash::HasHash;
use holochain_types::prelude::*;

pub fn dna_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            Ok(DnaInfo {
                name: ribosome.dna_def().name.clone(),
                hash: ribosome.dna_def().as_hash().clone(),
                properties: ribosome.dna_def().properties.clone()
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
    async fn invoke_import_dna_info_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let dna_info: DnaInfo =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "dna_info", ()).unwrap();
        assert_eq!(dna_info.name, String::from("test"));
    }
}