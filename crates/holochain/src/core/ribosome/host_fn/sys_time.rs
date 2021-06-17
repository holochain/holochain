use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn sys_time(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<core::time::Duration, WasmError> {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    Ok(since_the_epoch)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::fake_agent_pubkey_1;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_sys_time_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();


        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;
        let _: core::time::Duration =
            crate::call_test_ribosome!(host_access, TestWasm::SysTime, "sys_time", ());
    }
}
