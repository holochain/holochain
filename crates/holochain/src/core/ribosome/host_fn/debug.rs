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
    input: DebugMsg,
) -> RibosomeResult<()> {
    debug!(
        "{}:{}:{} {}",
        input.module_path(),
        input.file(),
        input.line(),
        input.msg()
    );
    Ok(())
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
        let input = debug_msg!(format!("ribosome debug {}", "works!"));

        let output: () = debug(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!((), output);
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
        let output: () =
            crate::call_test_ribosome!(host_access, TestWasm::Debug, "debug", ());
        assert_eq!(output, ());
    }
}
