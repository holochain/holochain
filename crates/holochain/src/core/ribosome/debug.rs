use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::debug::DebugMsg;
use sx_zome_types::DebugInput;
use sx_zome_types::DebugOutput;
use tracing::*;

pub fn debug(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: DebugInput,
) -> Result<DebugOutput, RibosomeError> {
    let msg: DebugMsg = input.into_inner();
    trace!(
        "{}:{}:{} {}",
        msg.module_path(),
        msg.file(),
        msg.line(),
        msg.msg()
    );
    Ok(DebugOutput::new(()))
}

#[cfg(test)]
pub mod wasm_test {
    use sx_zome_types::debug_msg;
    use sx_zome_types::DebugInput;
    use sx_zome_types::DebugOutput;

    #[test]
    fn ribosome_debug_test() {
        // this shows that debug is called but our line numbers will be messed up
        // the line numbers will show as coming from this test because we made the input here
        let output: DebugOutput = crate::call_test_ribosome!(
            "imports",
            "debug",
            DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")))
        );
        assert_eq!((), output.into_inner());
    }

    #[test]
    fn wasm_line_numbers_test() {
        // this shows that we can get line numbers out of wasm
        let output: DebugOutput = crate::call_test_ribosome!("debug", "debug", ());
        assert_eq!((), output.into_inner());
    }
}
