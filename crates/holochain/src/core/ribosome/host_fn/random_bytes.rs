use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use ring::rand::SecureRandom;
use std::sync::Arc;

/// return n crypto secure random bytes from the standard holochain crypto lib
pub fn random_bytes(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: RandomBytesInput,
) -> RibosomeResult<RandomBytesOutput> {
    let system_random = ring::rand::SystemRandom::new();
    let mut bytes = vec![0; input.into_inner() as _];
    system_random.fill(&mut bytes)?;

    Ok(RandomBytesOutput::new(Bytes::from(bytes)))
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
    use holochain_zome_types::RandomBytesInput;
    use holochain_zome_types::RandomBytesOutput;
    use std::convert::TryInto;
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
        const LEN: usize = 10;
        let input = RandomBytesInput::new(LEN.try_into().unwrap());

        let output: RandomBytesOutput =
            random_bytes(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        println!("{:?}", output);

        assert_ne!(&[0; LEN], output.into_inner().as_ref(),);
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get some random data out of the fn via. a wasm call
    async fn ribosome_random_bytes_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        const LEN: usize = 5;
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let output: RandomBytesOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::RandomBytes,
            "random_bytes",
            RandomBytesInput::new(5 as _)
        );

        assert_ne!(&[0; LEN], output.into_inner().as_ref(),);
    }
}
