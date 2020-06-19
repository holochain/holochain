use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::debug::DebugMsg;
use holochain_zome_types::DebugInput;
use holochain_zome_types::DebugOutput;
use std::sync::Arc;
use tracing::*;
use holochain_crypto::crypto_randombytes_buf;

pub async fn random_bytes(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: RandomBytesInput,
) -> RibosomeResult<RandomBytesOutput> {
    let len: u32 = input.into_inner();

    let mut buf: Vec<u8> = vec![0; len];

    crypto_randombytes_buf(&mut buf).await;

    Ok(
        RandomBytesOutput::new(buf)
    )
}

#[cfg(test)]
pub mod wasm_test {
    use holochain_zome_types::RandomBytesInput;
    use holochain_zome_types::RandomBytesOutput;

    #[tokio::test(threaded_scheduler)]
    async fn random_bytes_test() {
        let ribosome = WasmRibosomeFixturator::new(Unpredictable).next().unwrap();
        let host_context = HostContextFixturator::new(Unpredictable).next().unwrap();
        // cast a u8 here so we can keep the length manageable
        let len = U8Fixturator::new(Unpredictable).next().unwrap() as u32;
        let input = RandomBytesInput::new(len);

        let output: RandomBytesOutput = random_bytes(input).await;

        println!("{}", output);
    }

    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn ribosome_random_bytes_test() {
    //     // this shows that debug is called but our line numbers will be messed up
    //     // the line numbers will show as coming from this test because we made the input here
    //     let output: RandomBytesOutput = crate::call_test_ribosome!(
    //         TestWasm::Imports,
    //         "random_bytes",
    //         DebugInput::new(debug_msg!(format!("ribosome debug {}", "works!")))
    //     );
    //     assert_eq!(output, DebugOutput::new(()));
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn wasm_line_numbers_test() {
    //     // this shows that we can get line numbers out of wasm
    //     let output: DebugOutput = crate::call_test_ribosome!(TestWasm::Debug, "debug", ());
    //     assert_eq!(output, DebugOutput::new(()));
    // }
}
