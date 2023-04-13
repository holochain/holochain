use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

/// return n crypto secure random bytes from the standard holochain crypto lib
pub fn random_bytes(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: u32,
) -> Result<holochain_types::prelude::Bytes, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } => {
            let mut bytes = vec![0; input as _];
            getrandom::getrandom(&mut bytes)
                .map_err(|error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(error.to_string())).into()
                })?;

            Ok(holochain_types::prelude::Bytes::from(bytes))
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "random_bytes".into()
            )
            .to_string()
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::host_fn::random_bytes::random_bytes;

    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::HostContext;
    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    /// we can get some random data out of the fn directly
    async fn random_bytes_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let mut call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        call_context.host_context = HostContext::ZomeCall(fixt!(ZomeCallHostAccess));
        const LEN: u32 = 10;

        let output: holochain_zome_types::prelude::Bytes =
            random_bytes(Arc::new(ribosome), Arc::new(call_context), LEN).unwrap();

        println!("{:?}", output);

        assert_ne!(&[0; LEN as usize], output.as_ref(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get some random data out of the fn via. a wasm call
    async fn ribosome_random_bytes_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::RandomBytes).await;
        const LEN: u32 = 5;
        let output: hdk::prelude::Bytes = conductor.call(&alice, "random_bytes", LEN).await;

        assert_ne!(&vec![0; LEN as usize], &output.to_vec());
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get some random data out of the fn via. a wasm call
    async fn ribosome_rand_random_bytes_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::RandomBytes).await;
        const LEN: u32 = 5;
        let output: hdk::prelude::Bytes = conductor.call(&alice, "rand_random_bytes", LEN).await;

        assert_ne!(&vec![0; LEN as usize], &output.to_vec());
    }
}
