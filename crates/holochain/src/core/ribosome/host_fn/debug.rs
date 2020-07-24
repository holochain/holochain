use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::debug::DebugMsg;
use holochain_zome_types::DebugInput;
use holochain_zome_types::DebugOutput;
use std::sync::Arc;
use tracing::*;

pub fn debug(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
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
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::debug;
    use crate::core::state::workspace::Workspace;
    use crate::fixt::CallContextFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::debug_msg;
    use holochain_zome_types::DebugInput;
    use holochain_zome_types::DebugOutput;
    use std::sync::Arc;

    /// we can get an entry hash out of the fn directly
    #[tokio::test(threaded_scheduler)]
    async fn debug_test() {
        let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let input = DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")));

        let output: DebugOutput = debug(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!(DebugOutput::new(()), output);
    }

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_debug_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        // this shows that debug is called but our line numbers will be messed up
        // the line numbers will show as coming from this test because we made the input here
        let output: DebugOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::Imports,
            "debug",
            DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")))
        );
        assert_eq!(output, DebugOutput::new(()));
    }

    #[tokio::test(threaded_scheduler)]
    async fn wasm_line_numbers_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(&reader, &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        // this shows that we can get line numbers out of wasm
        let output: DebugOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Debug, "debug", ());
        assert_eq!(output, DebugOutput::new(()));
    }
}
