use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::AgentPubKeyExt;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn verify_signature(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: VerifySignature,
) -> Result<bool, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore_deterministic: Permission::Allow,
            ..
        } => Ok(tokio_helper::block_forever_on(async move {
            let VerifySignature {
                key,
                signature,
                data,
            } = input;
            key.verify_signature_raw(&signature, data.into()).await
        })),
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "verify_signature".into(),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::test_utils::fake_agent_pubkey_1;
    use holochain_zome_types::test_utils::fake_agent_pubkey_2;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_verify_signature_raw_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        // signatures should not change for a given pubkey
        for (name, expect, k, sig, data) in vec![
            (
                "first bit corrupted to a zero",
                false,
                alice_pubkey.clone(),
                [
                    0, 134, 114, 170, 178, 165, 117, 201, 98, 239, 41, 23, 223, 162, 103, 77, 44,
                    26, 215, 100, 248, 162, 55, 133, 60, 166, 234, 160, 229, 233, 46, 124, 6, 20,
                    254, 231, 246, 199, 115, 107, 248, 226, 7, 140, 177, 73, 81, 180, 150, 51, 146,
                    9, 17, 110, 244, 198, 74, 146, 200, 66, 155, 134, 46, 13,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                "valid sig",
                true,
                alice_pubkey.clone(),
                [
                    240, 134, 114, 170, 178, 165, 117, 201, 98, 239, 41, 23, 223, 162, 103, 77, 44,
                    26, 215, 100, 248, 162, 55, 133, 60, 166, 234, 160, 229, 233, 46, 124, 6, 20,
                    254, 231, 246, 199, 115, 107, 248, 226, 7, 140, 177, 73, 81, 180, 150, 51, 146,
                    9, 17, 110, 244, 198, 74, 146, 200, 66, 155, 134, 46, 13,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                "valid sig",
                true,
                bob_pubkey.clone(),
                [
                    93, 140, 255, 162, 86, 19, 120, 119, 201, 40, 251, 109, 22, 239, 184, 86, 55,
                    163, 10, 71, 223, 44, 197, 150, 179, 218, 5, 192, 116, 18, 235, 36, 203, 21,
                    195, 32, 63, 143, 43, 24, 40, 134, 208, 73, 223, 51, 166, 237, 130, 47, 251,
                    169, 7, 45, 185, 164, 89, 240, 67, 134, 168, 203, 158, 15,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                "last bit corrupted to zero",
                false,
                bob_pubkey.clone(),
                [
                    93, 140, 255, 162, 86, 19, 120, 119, 201, 40, 251, 109, 22, 239, 184, 86, 55,
                    163, 10, 71, 223, 44, 197, 150, 179, 218, 5, 192, 116, 18, 235, 36, 203, 21,
                    195, 32, 63, 143, 43, 24, 40, 134, 208, 73, 223, 51, 166, 237, 130, 47, 251,
                    169, 7, 45, 185, 164, 89, 240, 67, 134, 168, 203, 158, 0,
                ],
                vec![100_u8, 200_u8, 50_u8],
            ),
            (
                "first bit corrupted to a zero",
                false,
                alice_pubkey.clone(),
                [
                    0, 153, 68, 223, 254, 0, 113, 83, 152, 176, 155, 176, 198, 196, 59, 220, 199,
                    27, 215, 203, 8, 89, 108, 127, 130, 63, 45, 229, 225, 65, 127, 147, 207, 5, 52,
                    58, 65, 87, 10, 159, 248, 124, 177, 112, 91, 109, 200, 122, 99, 250, 129, 42,
                    207, 83, 42, 52, 101, 142, 110, 73, 91, 86, 117, 14,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                "valid sig",
                true,
                alice_pubkey,
                [
                    162, 153, 68, 223, 254, 0, 113, 83, 152, 176, 155, 176, 198, 196, 59, 220, 199,
                    27, 215, 203, 8, 89, 108, 127, 130, 63, 45, 229, 225, 65, 127, 147, 207, 5, 52,
                    58, 65, 87, 10, 159, 248, 124, 177, 112, 91, 109, 200, 122, 99, 250, 129, 42,
                    207, 83, 42, 52, 101, 142, 110, 73, 91, 86, 117, 14,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                "valid sig",
                true,
                bob_pubkey.clone(),
                [
                    83, 13, 130, 229, 254, 5, 115, 44, 148, 20, 3, 224, 231, 240, 8, 36, 28, 157,
                    16, 198, 86, 50, 129, 223, 66, 106, 78, 212, 110, 74, 214, 170, 106, 84, 55, 6,
                    193, 80, 222, 36, 205, 5, 30, 40, 1, 18, 219, 40, 87, 243, 12, 25, 20, 78, 102,
                    68, 139, 76, 224, 28, 221, 182, 142, 1,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
            (
                "last bit corrupted to zero",
                false,
                bob_pubkey,
                [
                    83, 13, 130, 229, 254, 5, 115, 44, 148, 20, 3, 224, 231, 240, 8, 36, 28, 157,
                    16, 198, 86, 50, 129, 223, 66, 106, 78, 212, 110, 74, 214, 170, 106, 84, 55, 6,
                    193, 80, 222, 36, 205, 5, 30, 40, 1, 18, 219, 40, 87, 243, 12, 25, 20, 78, 102,
                    68, 139, 76, 224, 28, 221, 182, 142, 0,
                ],
                vec![1_u8, 2_u8, 3_u8],
            ),
        ] {
            for _ in 0..2_usize {
                let output_raw: bool = conductor
                    .call(
                        &alice,
                        "verify_signature_raw",
                        VerifySignature::new_raw(k.clone(), sig.clone().into(), data.clone()),
                    )
                    .await;

                assert_eq!(expect, output_raw, "raw: {}, {}", name, k);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_verify_signature_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Sign).await;

        let _nothing: () = conductor
            .call(&alice, "verify_signature", alice_pubkey)
            .await;
    }
}
