use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_zome_types::SignInput;
use holochain_zome_types::SignOutput;
use std::sync::Arc;

pub fn sign(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: SignInput,
) -> RibosomeResult<SignOutput> {
    Ok(SignOutput::new(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            call_context
                .host_access
                .keystore()
                .sign(input.into_inner())
                .await
        })?,
    ))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk3::prelude::test_utils::fake_agent_pubkey_1;
    use hdk3::prelude::test_utils::fake_agent_pubkey_2;
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_sign_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();
        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess, Predictable);
        host_access.workspace = workspace_lock;

        // signatures should not change for a given pubkey
        for (k, expect) in vec![
            (
                fake_agent_pubkey_1(),
                vec![
                    251, 8, 85, 0, 99, 120, 224, 60, 96, 159, 135, 242, 73, 131, 132, 250, 252,
                    167, 103, 77, 136, 33, 9, 217, 84, 239, 213, 14, 85, 219, 111, 200, 26, 10, 54,
                    139, 128, 82, 188, 198, 123, 209, 29, 104, 2, 21, 40, 170, 118, 21, 137, 56,
                    55, 75, 129, 24, 233, 217, 113, 218, 147, 35, 91, 4,
                ],
            ),
            (
                fake_agent_pubkey_2(),
                vec![
                    213, 44, 10, 46, 76, 234, 139, 130, 96, 189, 1, 62, 5, 116, 106, 61, 151, 108,
                    110, 101, 61, 226, 208, 105, 7, 199, 65, 219, 100, 174, 58, 154, 199, 10, 147,
                    180, 37, 233, 49, 49, 249, 81, 110, 154, 63, 100, 75, 234, 64, 80, 64, 182,
                    118, 109, 139, 220, 63, 33, 179, 87, 213, 46, 3, 2,
                ],
            ),
        ] {
            for _ in 0..2 {
                let output: Signature = crate::call_test_ribosome!(
                    host_access,
                    TestWasm::Sign,
                    "sign",
                    hdk3::prelude::holochain_zome_types::zome_io::SignInput::new(
                        SignInput::new_raw(k.clone(), vec![100_u8, 200_u8, 50_u8],)
                    )
                );

                assert_eq!(expect, output.as_ref().to_vec());
            }
        }
    }
}
