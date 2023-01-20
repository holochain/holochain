use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::encode::blake2b_n;
use holo_hash::HasHash;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::prelude::*;
use std::sync::Arc;
use tiny_keccak::{Hasher, Keccak, Sha3};

pub fn hash(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: HashInput,
) -> Result<HashOutput, RuntimeError> {
    Ok(match input {
        HashInput::Entry(entry) => HashOutput::Entry(
            holochain_zome_types::entry::EntryHashed::from_content_sync(entry).into_hash(),
        ),
        HashInput::Action(action) => HashOutput::Action(
            holochain_zome_types::action::ActionHashed::from_content_sync(action).into_hash(),
        ),
        HashInput::Blake2B(data, output_len) => {
            HashOutput::Blake2B(blake2b_n(&data, output_len as usize).map_err(
                |e| -> RuntimeError { wasm_error!(WasmErrorInner::Host(e.to_string())).into() },
            )?)
        }
        HashInput::Keccak256(data) => HashOutput::Keccak256({
            let mut output = [0u8; 32];
            let mut hasher = Keccak::v256();
            hasher.update(data.as_ref());
            hasher.finalize(&mut output);
            output.into()
        }),
        HashInput::Sha3256(data) => HashOutput::Sha3256({
            let mut output = [0u8; 32];
            let mut hasher = Sha3::v256();
            hasher.update(data.as_ref());
            hasher.finalize(&mut output);
            output.into()
        }),
        _ => {
            return Err(wasm_error!(WasmErrorInner::Host(format!(
                "Unimplemented hashing algorithm {:?}",
                input
            )))
            .into())
        }
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;
    use crate::core::ribosome::host_fn::hash::hash;

    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holo_hash::EntryHash;
    use holochain_wasm_test_utils::TestWasm;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn directly
    async fn hash_test() {
        let ribosome = Arc::new(
            RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
                .next()
                .unwrap(),
        );
        let call_context = Arc::new(
            CallContextFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap(),
        );
        let entry_input =
            HashInput::Entry(EntryFixturator::new(::fixt::Predictable).next().unwrap());

        let entry_output: EntryHash = match hash(
            Arc::clone(&ribosome),
            Arc::clone(&call_context),
            entry_input,
        )
        .unwrap()
        {
            HashOutput::Entry(output) => output,
            _ => unreachable!(),
        };

        assert_eq!(*entry_output.hash_type(), holo_hash::hash_type::Entry);

        let action_input = HashInput::Action(fixt!(Action));

        let action_output: ActionHash = match hash(
            Arc::clone(&ribosome),
            Arc::clone(&call_context),
            action_input,
        )
        .unwrap()
        {
            HashOutput::Action(output) => output,
            _ => unreachable!(),
        };

        assert_eq!(*action_output.hash_type(), holo_hash::hash_type::Action);

        let blake2b_input = HashInput::Blake2B(vec![1, 2, 3], 5);

        let blake2b_output: Vec<u8> = match hash(
            Arc::clone(&ribosome),
            Arc::clone(&call_context),
            blake2b_input,
        )
        .unwrap()
        {
            HashOutput::Blake2B(output) => output,
            _ => unreachable!(),
        };

        // a lovely 5 byte blake2b hash.
        assert_eq!(blake2b_output, vec![89, 41, 133, 48, 237,]);

        let keccak_input = HashInput::Keccak256(vec![0]);

        let keccak_output: Hash256Bits = match hash(
            Arc::clone(&ribosome),
            Arc::clone(&call_context),
            keccak_input,
        )
        .unwrap()
        {
            HashOutput::Keccak256(output) => output,
            _ => unreachable!(),
        };

        // https://emn178.github.io/online-tools/keccak_256.html
        // bc36789e7a1e281436464229828f817d6612f7b477d66591ff96a9e064bcc98a
        assert_eq!(
            keccak_output,
            [
                0xbc, 0x36, 0x78, 0x9e, 0x7a, 0x1e, 0x28, 0x14, 0x36, 0x46, 0x42, 0x29, 0x82, 0x8f,
                0x81, 0x7d, 0x66, 0x12, 0xf7, 0xb4, 0x77, 0xd6, 0x65, 0x91, 0xff, 0x96, 0xa9, 0xe0,
                0x64, 0xbc, 0xc9, 0x8a
            ]
            .into(),
        );

        let sha3_input = HashInput::Sha3256(vec![0]);

        let sha3_output: Hash256Bits =
            match hash(Arc::clone(&ribosome), Arc::clone(&call_context), sha3_input).unwrap() {
                HashOutput::Sha3256(output) => output,
                _ => unreachable!(),
            };

        // https://emn178.github.io/online-tools/sha3_256.html
        // 5d53469f20fef4f8eab52b88044ede69c77a6a68a60728609fc4a65ff531e7d0
        assert_eq!(
            sha3_output,
            [
                0x5d, 0x53, 0x46, 0x9f, 0x20, 0xfe, 0xf4, 0xf8, 0xea, 0xb5, 0x2b, 0x88, 0x04, 0x4e,
                0xde, 0x69, 0xc7, 0x7a, 0x6a, 0x68, 0xa6, 0x07, 0x28, 0x60, 0x9f, 0xc4, 0xa6, 0x5f,
                0xf5, 0x31, 0xe7, 0xd0
            ]
            .into(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn via. a wasm call
    async fn ribosome_hash_entry_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::HashEntry).await;
        let input = EntryFixturator::new(::fixt::Predictable).next().unwrap();
        let output: EntryHash = conductor.call(&alice, "hash_entry", input).await;
        assert_eq!(*output.hash_type(), holo_hash::hash_type::Entry);

        let entry_hash_output: EntryHash = conductor
            .call(&alice, "twenty_three_degrees_entry_hash", ())
            .await;

        let hash_output: EntryHash = conductor
            .call(&alice, "twenty_three_degrees_hash", ())
            .await;

        assert_eq!(entry_hash_output, hash_output);
    }
}
