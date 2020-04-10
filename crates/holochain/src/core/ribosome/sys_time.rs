use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::SysTimeInput;
use sx_zome_types::SysTimeOutput;

pub fn sys_time(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SysTimeInput,
) -> SysTimeOutput {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    SysTimeOutput::new(since_the_epoch)
}

#[cfg(all(test, feature = "wasmtest"))]
pub mod wasm_test {
    use sx_zome_types::zome_io::SysTimeOutput;
    use sx_zome_types::SysTimeInput;

    #[test]
    fn invoke_import_sys_time_test() {
        let _: SysTimeOutput = crate::call_test_ribosome!("imports", "sys_time", SysTimeInput::new(()));
    }
}
