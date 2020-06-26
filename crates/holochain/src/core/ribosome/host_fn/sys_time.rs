use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::SysTimeInput;
use holochain_zome_types::SysTimeOutput;
use std::sync::Arc;

pub fn sys_time(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: SysTimeInput,
) -> RibosomeResult<SysTimeOutput> {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    Ok(SysTimeOutput::new(since_the_epoch))
}

#[cfg(test)]
pub mod wasm_test {
    // use holochain_wasm_test_utils::TestWasm;
    // use holochain_zome_types::{SysTimeInput, SysTimeOutput};
    // use holochain_state::env::ReadManager;
    // use crate::core::state::workspace::Workspace;

    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn invoke_import_sys_time_test() {
    //     let env = holochain_state::test_utils::test_cell_env();
    //     let dbs = env.dbs().await;
    //     let env_ref = env.guard().await;
    //     let reader = env_ref.reader().unwrap();
    //     let mut workspace = crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
    //
    //     let _: SysTimeOutput =
    //         crate::call_test_ribosome!(workspace, TestWasm::Imports, "sys_time", SysTimeInput::new(()));
    // }
}
