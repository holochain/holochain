use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::DebugInput;
use sx_zome_types::DebugOutput;

pub fn debug(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: DebugInput,
) -> DebugOutput {
    println!("{}", input.inner());
    DebugOutput::new(())
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod test {

    use crate::core::ribosome::wasm_test::test_ribosome;
    use crate::core::ribosome::wasm_test::zome_invocation_from_names;
    use crate::core::ribosome::RibosomeT;
    use std::convert::TryInto;
    use sx_types::shims::SourceChainCommitBundle;
    use sx_zome_types::DebugInput;

    #[test]
    fn invoke_import_debug_test() {
        let ribosome = test_ribosome();

        let invocation = zome_invocation_from_names(
            "imports",
            "debug",
            DebugInput::new(format!("debug {:?}", "works!"))
                .try_into()
                .unwrap(),
        );

        ribosome
            .call_zome_function(&mut SourceChainCommitBundle::default(), invocation)
            .unwrap();
    }
}
