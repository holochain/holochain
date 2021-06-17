use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn sign(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: Sign,
) -> Result<Signature, WasmError> {
    tokio_helper::block_forever_on(async move {
        call_context.host_access.keystore().sign(input).await
    })
    .map_err(|keystore_error| WasmError::Host(keystore_error.to_string()))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::test_utils::fake_agent_pubkey_1;
    use hdk::prelude::test_utils::fake_agent_pubkey_2;
    use hdk::prelude::*;
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_sign_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();

        let mut host_access = fixt!(ZomeCallHostAccess, Predictable);
        host_access.workspace = workspace;

        // signatures should not change for a given pubkey
        for (k, data, expect) in vec![
            (
                fake_agent_pubkey_1(),
                vec![100_u8, 200_u8, 50_u8],
                vec![
                    251, 8, 85, 0, 99, 120, 224, 60, 96, 159, 135, 242, 73, 131, 132, 250, 252,
                    167, 103, 77, 136, 33, 9, 217, 84, 239, 213, 14, 85, 219, 111, 200, 26, 10, 54,
                    139, 128, 82, 188, 198, 123, 209, 29, 104, 2, 21, 40, 170, 118, 21, 137, 56,
                    55, 75, 129, 24, 233, 217, 113, 218, 147, 35, 91, 4,
                ],
            ),
            (
                fake_agent_pubkey_2(),
                vec![100_u8, 200_u8, 50_u8],
                vec![
                    213, 44, 10, 46, 76, 234, 139, 130, 96, 189, 1, 62, 5, 116, 106, 61, 151, 108,
                    110, 101, 61, 226, 208, 105, 7, 199, 65, 219, 100, 174, 58, 154, 199, 10, 147,
                    180, 37, 233, 49, 49, 249, 81, 110, 154, 63, 100, 75, 234, 64, 80, 64, 182,
                    118, 109, 139, 220, 63, 33, 179, 87, 213, 46, 3, 2,
                ],
            ),
            (
                fake_agent_pubkey_1(),
                vec![1_u8, 2_u8, 3_u8],
                vec![
                    164, 81, 93, 22, 145, 161, 253, 101, 252, 0, 68, 177, 223, 131, 66, 2, 123,
                    156, 9, 83, 126, 246, 150, 41, 153, 251, 153, 150, 185, 218, 134, 41, 16, 50,
                    112, 122, 51, 77, 206, 0, 100, 135, 228, 79, 104, 124, 238, 165, 49, 41, 172,
                    36, 121, 38, 176, 49, 83, 250, 98, 179, 152, 112, 82, 2,
                ],
            ),
            (
                fake_agent_pubkey_2(),
                vec![1_u8, 2_u8, 3_u8],
                vec![
                    118, 23, 120, 77, 58, 149, 72, 23, 197, 20, 213, 185, 189, 45, 221, 90, 198,
                    231, 214, 97, 10, 172, 9, 99, 182, 38, 41, 34, 203, 199, 117, 33, 43, 57, 247,
                    157, 22, 29, 64, 78, 68, 5, 60, 126, 195, 247, 128, 225, 94, 225, 26, 214, 203,
                    169, 76, 165, 28, 151, 224, 218, 141, 47, 92, 11,
                ],
            ),
        ] {
            for _ in 0..2 {
                let output: Signature = crate::call_test_ribosome!(
                    host_access,
                    TestWasm::Sign,
                    "sign",
                    Sign::new_raw(k.clone(), data.clone())
                );

                assert_eq!(expect, output.as_ref().to_vec());
            }
        }
    }
}
