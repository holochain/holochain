use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::globals::ZomeGlobals;
use holochain_zome_types::GlobalsInput;
use holochain_zome_types::GlobalsOutput;
use std::convert::TryFrom;
use std::sync::Arc;

pub fn globals(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GlobalsInput,
) -> RibosomeResult<GlobalsOutput> {
    Ok(GlobalsOutput::new(ZomeGlobals {
        dna_name: ribosome.dna_file().dna().name.clone(),
        zome_name: host_context.zome_name.clone(),
        agent_address: "".into(),                           // @TODO
        agent_id_str: "".into(),                            // @TODO
        agent_initial_hash: "".into(),                      // @TODO
        agent_latest_hash: "".into(),                       // @TODO
        dna_address: "".into(),                             // @TODO
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
        public_token: "".into(),                            // @TODO
    }))
}

#[cfg(test)]
pub mod test {
    use crate::core::state::workspace::Workspace;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::GlobalsInput;
    use holochain_zome_types::GlobalsOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_globals_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let globals: GlobalsOutput = crate::call_test_ribosome!(
            raw_workspace,
            TestWasm::Imports,
            "globals",
            GlobalsInput::new(())
        );
        assert_eq!(globals.inner_ref().dna_name, "test",);
    }
}
