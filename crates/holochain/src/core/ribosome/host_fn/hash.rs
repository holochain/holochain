use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::encode::blake2b_n;
use holo_hash::HasHash;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::prelude::*;
use std::sync::Arc;

pub fn hash(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: HashInput,
) -> Result<HashOutput, WasmError> {
    Ok(match input {
        HashInput::Entry(entry) => HashOutput::Entry(
            holochain_zome_types::entry::EntryHashed::from_content_sync(entry).into_hash(),
        ),
        HashInput::Header(header) => HashOutput::Header(
            holochain_zome_types::header::HeaderHashed::from_content_sync(header).into_hash(),
        ),
        HashInput::Blake2B(data, output_len) => HashOutput::Blake2B(
            blake2b_n(&data, output_len as usize).map_err(|e| WasmError::Host(e.to_string()))?,
        ),
        _ => {
            return Err(WasmError::Host(format!(
                "Unimplemented hashing algorithm {:?}",
                input
            )))
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
    use hdk::hash_path::path::Component;
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

        let header_input = HashInput::Header(fixt!(Header));

        let header_output: HeaderHash = match hash(
            Arc::clone(&ribosome),
            Arc::clone(&call_context),
            header_input,
        )
        .unwrap()
        {
            HashOutput::Header(output) => output,
            _ => unreachable!(),
        };

        assert_eq!(*header_output.hash_type(), holo_hash::hash_type::Header);

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

    #[tokio::test(flavor = "multi_thread")]
    /// the hash path underlying anchors wraps entry_hash
    async fn ribosome_hash_path_pwd_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::HashPath).await;
        let input = "foo.bar".to_string();
        let output: EntryHash = conductor.call(&alice, "path_entry_hash", input).await;

        let expected_path =
            hdk::hash_path::path::Path::from(vec![Component::from("foo"), Component::from("bar")]);

        let path_hash = holochain_zome_types::entry::EntryHashed::from_content_sync(
            Entry::try_from(expected_path).unwrap(),
        )
        .into_hash();

        let path_entry_hash = holochain_zome_types::entry::EntryHashed::from_content_sync(
            Entry::try_from(PathEntry::new(path_hash)).unwrap(),
        )
        .into_hash();

        assert_eq!(path_entry_hash.into_inner(), output.into_inner(),);
    }
}
