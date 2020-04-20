use super::HostContext;
use super::WasmRibosome;
use holochain_serialized_bytes::SerializedBytes;
use std::convert::TryFrom;
use std::sync::Arc;
use sx_zome_types::globals::ZomeGlobals;
use sx_zome_types::GlobalsInput;
use sx_zome_types::GlobalsOutput;

pub fn globals(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GlobalsInput,
) -> GlobalsOutput {
    GlobalsOutput::new(ZomeGlobals {
        agent_address: "".into(),                           // @TODO
        agent_id_str: "".into(),                            // @TODO
        agent_initial_hash: "".into(),                      // @TODO
        agent_latest_hash: "".into(),                       // @TODO
        dna_address: "".into(),                             // @TODO
        dna_name: "".into(),                                // @TODO
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
        public_token: "".into(),                            // @TODO
    })
}

#[cfg(test)]
pub mod test {
    use sx_zome_types::GlobalsInput;
    use sx_zome_types::GlobalsOutput;

    #[test]
    fn invoke_import_globals_test() {
        let globals: GlobalsOutput =
            crate::call_test_ribosome!("imports", "globals", GlobalsInput::new(()));
        assert_eq!(globals.inner_ref().dna_name, "test",);
    }
}
