use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::AgentPubKeyExt;
use holochain_zome_types::VerifySignatureInput;
use holochain_zome_types::VerifySignatureOutput;
use std::sync::Arc;

pub fn verify_signature(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: VerifySignatureInput,
) -> RibosomeResult<VerifySignatureOutput> {
    let input = input.into_inner();
    Ok(VerifySignatureOutput::new(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            input
                .key
                .verify_signature_raw(input.as_ref(), input.as_data_ref().bytes())
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
    async fn ribosome_verify_signature_test() {
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
        for (expect, k, sig, data) in vec![
            (
                // first bit corrupted to a zero
                false,
                fake_agent_pubkey_1(),
                vec![
                    0, 8, 85, 0, 99, 120, 224, 60, 96, 159, 135, 242, 73, 131, 132, 250, 252, 167,
                    103, 77, 136, 33, 9, 217, 84, 239, 213, 14, 85, 219, 111, 200, 26, 10, 54, 139,
                    128, 82, 188, 198, 123, 209, 29, 104, 2, 21, 40, 170, 118, 21, 137, 56, 55, 75,
                    129, 24, 233, 217, 113, 218, 147, 35, 91, 4,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                // valid sig
                true,
                fake_agent_pubkey_1(),
                vec![
                    251, 8, 85, 0, 99, 120, 224, 60, 96, 159, 135, 242, 73, 131, 132, 250, 252,
                    167, 103, 77, 136, 33, 9, 217, 84, 239, 213, 14, 85, 219, 111, 200, 26, 10, 54,
                    139, 128, 82, 188, 198, 123, 209, 29, 104, 2, 21, 40, 170, 118, 21, 137, 56,
                    55, 75, 129, 24, 233, 217, 113, 218, 147, 35, 91, 4,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                // valid sig
                true,
                fake_agent_pubkey_2(),
                vec![
                    213, 44, 10, 46, 76, 234, 139, 130, 96, 189, 1, 62, 5, 116, 106, 61, 151, 108,
                    110, 101, 61, 226, 208, 105, 7, 199, 65, 219, 100, 174, 58, 154, 199, 10, 147,
                    180, 37, 233, 49, 49, 249, 81, 110, 154, 63, 100, 75, 234, 64, 80, 64, 182,
                    118, 109, 139, 220, 63, 33, 179, 87, 213, 46, 3, 2,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                // last bit corrupted to zero
                false,
                fake_agent_pubkey_2(),
                vec![
                    213, 44, 10, 46, 76, 234, 139, 130, 96, 189, 1, 62, 5, 116, 106, 61, 151, 108,
                    110, 101, 61, 226, 208, 105, 7, 199, 65, 219, 100, 174, 58, 154, 199, 10, 147,
                    180, 37, 233, 49, 49, 249, 81, 110, 154, 63, 100, 75, 234, 64, 80, 64, 182,
                    118, 109, 139, 220, 63, 33, 179, 87, 213, 46, 3, 0,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                // first bit corrupted to a zero
                false,
                fake_agent_pubkey_1(),
                vec![
                    0, 81, 93, 22, 145, 161, 253, 101, 252, 0, 68, 177, 223, 131, 66, 2, 123, 156,
                    9, 83, 126, 246, 150, 41, 153, 251, 153, 150, 185, 218, 134, 41, 16, 50, 112,
                    122, 51, 77, 206, 0, 100, 135, 228, 79, 104, 124, 238, 165, 49, 41, 172, 36,
                    121, 38, 176, 49, 83, 250, 98, 179, 152, 112, 82, 2,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                // valid sig
                true,
                fake_agent_pubkey_1(),
                vec![
                    164, 81, 93, 22, 145, 161, 253, 101, 252, 0, 68, 177, 223, 131, 66, 2, 123,
                    156, 9, 83, 126, 246, 150, 41, 153, 251, 153, 150, 185, 218, 134, 41, 16, 50,
                    112, 122, 51, 77, 206, 0, 100, 135, 228, 79, 104, 124, 238, 165, 49, 41, 172,
                    36, 121, 38, 176, 49, 83, 250, 98, 179, 152, 112, 82, 2,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                // valid sig
                true,
                fake_agent_pubkey_2(),
                vec![
                    118, 23, 120, 77, 58, 149, 72, 23, 197, 20, 213, 185, 189, 45, 221, 90, 198,
                    231, 214, 97, 10, 172, 9, 99, 182, 38, 41, 34, 203, 199, 117, 33, 43, 57, 247,
                    157, 22, 29, 64, 78, 68, 5, 60, 126, 195, 247, 128, 225, 94, 225, 26, 214, 203,
                    169, 76, 165, 28, 151, 224, 218, 141, 47, 92, 11,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                // last bit corrupted to zero
                false,
                fake_agent_pubkey_2(),
                vec![
                    118, 23, 120, 77, 58, 149, 72, 23, 197, 20, 213, 185, 189, 45, 221, 90, 198,
                    231, 214, 97, 10, 172, 9, 99, 182, 38, 41, 34, 203, 199, 117, 33, 43, 57, 247,
                    157, 22, 29, 64, 78, 68, 5, 60, 126, 195, 247, 128, 225, 94, 225, 26, 214, 203,
                    169, 76, 165, 28, 151, 224, 218, 141, 47, 92, 0,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
        ] {
            for _ in 0..2 {
                let output: VerifySignatureOutput = crate::call_test_ribosome!(
                    host_access,
                    TestWasm::Sign,
                    "verify_signature",
                    hdk3::prelude::holochain_zome_types::zome_io::VerifySignatureInput::new(
                        VerifySignature::new_raw(k.clone(), sig.clone().into(), data.clone())
                    )
                );

                assert_eq!(expect, output.into_inner());
            }
        }
    }
}
