use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::debug::DebugMsg;
use holochain_zome_types::DebugInput;
use holochain_zome_types::DebugOutput;
use std::sync::Arc;
use tracing::*;

pub async fn debug(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: DebugInput,
) -> RibosomeResult<DebugOutput> {
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
    use holochain_zome_types::debug_msg;
    use holochain_zome_types::DebugInput;
    use holochain_zome_types::DebugOutput;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_debug_test() {
        // this shows that debug is called but our line numbers will be messed up
        // the line numbers will show as coming from this test because we made the input here
        let output: DebugOutput = crate::call_test_ribosome!(
            "imports",
            "debug",
            DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")))
        );
        assert_eq!(output, DebugOutput::new(()));
    }

    #[tokio::test(threaded_scheduler)]
    async fn wasm_line_numbers_test() {
        // this shows that we can get line numbers out of wasm
        let output: DebugOutput = crate::call_test_ribosome!("debug", "debug", ());
        assert_eq!(output, DebugOutput::new(()));
    }
}
