use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::SysTimeInput;
use holochain_zome_types::SysTimeOutput;
use std::sync::Arc;

pub fn sys_time(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: SysTimeInput,
) -> RibosomeResult<SysTimeOutput> {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    Ok(SysTimeOutput::new(since_the_epoch))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::{SysTimeInput, SysTimeOutput};

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_sys_time_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(env.clone().into(), &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        let _: SysTimeOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::Imports,
            "sys_time",
            SysTimeInput::new(())
        );
    }
}
