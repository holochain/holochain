use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::NoopInput;
use holochain_zome_types::NoopOutput;
use std::sync::Arc;
use tracing::*;

pub async fn noop(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: NoopInput,
) -> RibosomeResult<NoopOutput> {
    trace!("noop! (likely attempted to call side effects in a context where that is not allowed)");
    Ok(NoopOutput::new(()))
}

#[cfg(test)]
pub mod wasm_test {
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::NoopInput;
    use holochain_zome_types::NoopOutput;

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn ribosome_noop_test() {
        // this shows that debug is called but our line numbers will be messed up
        // the line numbers will show as coming from this test because we made the input here
        let output: NoopOutput =
            crate::call_test_ribosome!(TestWasm::Imports, "noop", NoopInput::new(()));
        assert_eq!(output, NoopOutput::new(()));
    }
}
