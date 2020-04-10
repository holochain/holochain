use super::HostContext;
use super::WasmRibosome;
use holochain_serialized_bytes::SerializedBytes;
use std::convert::TryFrom;
use std::sync::Arc;
use sx_zome_types::globals::ZomeGlobals;
use sx_zome_types::GlobalsInput;
use sx_zome_types::GlobalsOutput;

pub fn globals(
    ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: GlobalsInput,
) -> GlobalsOutput {
    GlobalsOutput::new(ZomeGlobals {
        agent_address: "".into(),      // @TODO
        agent_id_str: "".into(),       // @TODO
        agent_initial_hash: "".into(), // @TODO
        agent_latest_hash: "".into(),  // @TODO
        dna_address: "".into(),        // @TODO
        dna_name: ribosome.dna.name.clone(),
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
        public_token: "".into(),                            // @TODO
    })
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::now;
    use crate::core::ribosome::wasm_test::test_ribosome;
    use crate::core::ribosome::wasm_test::zome_invocation_from_names;
    use crate::core::ribosome::RibosomeT;
    use std::convert::TryFrom;
    use std::convert::TryInto;
    use sx_types::nucleus::ZomeInvocationResponse;
    use sx_types::prelude::SerializedBytes;
    use sx_types::shims::SourceChainCommitBundle;
    use sx_zome_types::GlobalsInput;
    use sx_zome_types::GlobalsOutput;

    #[test]
    fn invoke_import_globals_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "globals",
            GlobalsInput::new(()).try_into().unwrap(),
        );

        let output_sb: SerializedBytes = match ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
        {
            Ok(ZomeInvocationResponse::ZomeApiFn(guest_output)) => guest_output.inner(),
            _ => unreachable!(),
        };
        let output = GlobalsOutput::try_from(output_sb).unwrap().inner();

        assert_eq!(output.dna_name, "test",);

        let ribosome = test_ribosome();
        let invocation = zome_invocation_from_names(
            "imports",
            "globals",
            GlobalsInput::new(()).try_into().unwrap(),
        );
        let t0 = now();
        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
        let t1 = now();
    }
}
