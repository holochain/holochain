use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::DnaInfo;
use std::sync::Arc;

pub fn dna_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            bindings_deterministic: Permission::Allow,
            ..
        } => Ok(DnaInfo {
            name: ribosome.dna_def().name.clone(),
            hash: ribosome.dna_def().as_hash().clone(),
            properties: ribosome.dna_def().properties.clone(),
            zome_names: ribosome
                .dna_def()
                .zomes
                .iter()
                .map(|(zome_name, _zome_def)| zome_name.to_owned())
                .collect(),
        }),
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "dna_info".into(),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_dna_info_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::ZomeInfo).await;

        let dna_info: DnaInfo = conductor.call(&alice, "dna_info", ()).await;
        assert_eq!(dna_info.name, String::from("test"));
    }
}
