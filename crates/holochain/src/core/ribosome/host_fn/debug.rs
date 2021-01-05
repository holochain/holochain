use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use std::sync::Arc;
use tracing::*;

#[instrument(skip(_ribosome, _call_context, input))]
pub fn debug(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: DebugInput,
) -> RibosomeResult<DebugOutput> {
    let msg: DebugMsg = input.into_inner();
    debug!(
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

    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::debug_msg;
    use holochain_zome_types::DebugInput;
    use holochain_zome_types::DebugOutput;
    use std::sync::Arc;

    /// we can get an entry hash out of the fn directly
    #[tokio::test(threaded_scheduler)]
    async fn debug_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let input = DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")));

        let output: DebugOutput = debug(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!(DebugOutput::new(()), output);
    }

    #[tokio::test(threaded_scheduler)]
    async fn wasm_line_numbers_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;

        // this shows that we can get line numbers out of wasm
        let output: DebugOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Debug, "debug", ());
        assert_eq!(output, DebugOutput::new(()));
    }
}
