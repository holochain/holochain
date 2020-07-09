use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::globals::ZomeInfo;
use holochain_zome_types::ZomeInfoInput;
use holochain_zome_types::ZomeInfoOutput;
use std::convert::TryFrom;
use std::sync::Arc;

pub fn zome_info(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: ZomeInfoInput,
) -> RibosomeResult<ZomeInfoOutput> {
    Ok(ZomeInfoOutput::new(ZomeInfo {
        dna_name: ribosome.dna_file().dna().name.clone(),
        zome_name: host_context.zome_name.clone(),
        dna_address: "".into(),                             // @TODO
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
        public_token: "".into(),                            // @TODO
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::state::workspace::Workspace;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_zome_info_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let zome_info: ZomeInfoOutput = crate::call_test_ribosome!(
            raw_workspace,
            TestWasm::Imports,
            "zome_info",
            ZomeInfoInput::new(())
        );
        assert_eq!(zome_info.inner_ref().dna_name, "test",);
    }
}
