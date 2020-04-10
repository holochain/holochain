use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::SleepInput;
use sx_zome_types::SleepOutput;

pub fn sleep(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: SleepInput,
) -> SleepOutput {
    std::thread::sleep(input.inner());
    SleepOutput::new(())
}

#[cfg(test)]
pub mod wasm_test {
    use sx_zome_types::SleepInput;
    use sx_zome_types::SleepOutput;

    #[test]
    fn invoke_import_sleep_test() {
        let _: SleepOutput = crate::call_test_ribosome!(
            "imports",
            "sleep",
            SleepInput::new(std::time::Duration::from_millis(3))
        );
    }
}
