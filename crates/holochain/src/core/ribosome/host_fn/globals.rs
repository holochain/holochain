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

pub async fn globals(
    ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GlobalsInput,
) -> RibosomeResult<GlobalsOutput> {
    Ok(GlobalsOutput::new(ZomeGlobals {
        dna_name: ribosome.dna_file().dna().name.clone(),
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
    use holochain_zome_types::GlobalsInput;
    use holochain_zome_types::GlobalsOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_globals_test() {
        let globals: GlobalsOutput =
            crate::call_test_ribosome!("imports", "globals", GlobalsInput::new(()));
        assert_eq!(globals.inner_ref().dna_name, "test",);
    }
}
