use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use ring::rand::SecureRandom;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

/// return n crypto secure random bytes from the standard holochain crypto lib
pub fn random_bytes(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: u32,
) -> Result<Bytes, WasmError> {
    match HostFnAccess::from(&call_context.host_access()) {
        HostFnAccess{ non_determinism: Permission::Allow, .. } => {
            let system_random = ring::rand::SystemRandom::new();
            let mut bytes = vec![0; input as _];
            system_random
                .fill(&mut bytes)
                .map_err(|ring_unspecified_error| WasmError::Host(ring_unspecified_error.to_string()))?;

            Ok(Bytes::from(bytes))
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::host_fn::random_bytes::random_bytes;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::fake_agent_pubkey_1;
    use std::sync::Arc;
    use crate::core::ribosome::HostAccess;

    #[tokio::test(flavor = "multi_thread")]
    /// we can get some random data out of the fn directly
    async fn random_bytes_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let mut call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        call_context.host_access = HostAccess::ZomeCall(fixt!(ZomeCallHostAccess));
        const LEN: u32 = 10;

        let output: holochain_zome_types::prelude::Bytes =
            random_bytes(Arc::new(ribosome), Arc::new(call_context), LEN).unwrap();

        println!("{:?}", output);

        assert_ne!(&[0; LEN as usize], output.as_ref(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get some random data out of the fn via. a wasm call
    async fn ribosome_random_bytes_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();

        const LEN: u32 = 5;
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;
        let output: hdk::prelude::Bytes =
            crate::call_test_ribosome!(host_access, TestWasm::RandomBytes, "random_bytes", LEN);

        assert_ne!(&vec![0; LEN as usize], &output.to_vec());
    }
}
