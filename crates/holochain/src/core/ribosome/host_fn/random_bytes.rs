use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use ring::rand::SecureRandom;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

/// return n crypto secure random bytes from the standard holochain crypto lib
pub fn random_bytes(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: u32,
) -> Result<Bytes, WasmError> {
    let system_random = ring::rand::SystemRandom::new();
    let mut bytes = vec![0; input as _];
    system_random.fill(&mut bytes).map_err(|ring_unspecified_error|
        WasmError::Host(ring_unspecified_error.to_string())
    )?;

    Ok(Bytes::from(bytes))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::host_fn::random_bytes::random_bytes;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    /// we can get some random data out of the fn directly
    async fn random_bytes_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        const LEN: u32 = 10;

        let output: holochain_zome_types::prelude::Bytes =
            random_bytes(Arc::new(ribosome), Arc::new(call_context), LEN).unwrap();

        println!("{:?}", output);

        assert_ne!(&[0; LEN as usize], output.as_ref(),);
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get some random data out of the fn via. a wasm call
    async fn ribosome_random_bytes_test() {
        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        const LEN: u32 = 5;
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let output: hdk::prelude::Bytes = crate::call_test_ribosome!(
            host_access,
            TestWasm::RandomBytes,
            "random_bytes",
            LEN
        );

        assert_ne!(&vec![0; LEN as usize], &output.to_vec());
    }
}
