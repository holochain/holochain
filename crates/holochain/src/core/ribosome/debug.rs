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
pub mod wasm_test {
    use sx_zome_types::DebugInput;
    use sx_zome_types::DebugOutput;

    #[test]
    fn ribosome_debug_test() {
        let _: DebugOutput = crate::call_test_ribosome!(
            "imports",
            "debug",
            DebugInput::new(format!("ribosome debug {}", "works!"))
        );
    }
}
