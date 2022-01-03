use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn hash_entry(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: Entry,
) -> Result<EntryHash, WasmError> {
    let entry_hash = holochain_zome_types::entry::EntryHashed::from_content_sync(input).into_hash();

    Ok(entry_hash)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;
    use crate::core::ribosome::host_fn::hash_entry::hash_entry;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holo_hash::EntryHash;
    use holochain_wasm_test_utils::TestWasm;
    use std::sync::Arc;
    use hdk::hash_path::path::Component;
    use hdk::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn directly
    async fn hash_entry_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let input = EntryFixturator::new(::fixt::Predictable).next().unwrap();

        let output: EntryHash =
            hash_entry(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!(*output.hash_type(), holo_hash::hash_type::Entry);
    }

    #[tokio::test(flavor = "multi_thread")]
    /// we can get an entry hash out of the fn via. a wasm call
    async fn ribosome_hash_entry_test() {
        let input = EntryFixturator::new(::fixt::Predictable).next().unwrap();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashEntry, "hash_entry", input).unwrap();
        assert_eq!(*output.hash_type(), holo_hash::hash_type::Entry);

        let entry_hash_output: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashEntry,
            "twenty_three_degrees_entry_hash",
            ()
        ).unwrap();

        let hash_output: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashEntry,
            "twenty_three_degrees_hash",
            ()
        ).unwrap();

        assert_eq!(entry_hash_output, hash_output);
    }

    #[tokio::test(flavor = "multi_thread")]
    /// the hash path underlying anchors wraps entry_hash
    async fn ribosome_hash_path_pwd_test() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let input = "foo.bar".to_string();
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashPath, "path_entry_hash", input).unwrap();

        let expected_path = hdk::hash_path::path::Path::from(vec![
            Component::from("foo"),
            Component::from("bar")
        ]);

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
